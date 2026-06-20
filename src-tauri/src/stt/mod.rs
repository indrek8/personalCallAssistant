//! Local STT pipeline (technical-design.md §5).
//!
//! A single dedicated thread owns the Whisper state + two VAD [`Segmenter`]s
//! (You / Remote). It consumes 16 kHz mono tagged chunks teed from capture,
//! segments each side into utterances, transcribes them with whisper, and emits
//! a [`TranscriptEntry`] per utterance — appended incrementally to
//! `transcript.jsonl` (ground truth) **and** forwarded over a channel for live
//! display (PR3 turns that into the `transcript-entry` event).
//!
//! Whisper inference is sequential and blocking; while it runs, incoming chunks
//! queue in the channel (the WAV in PR1 is the ground truth, so nothing is
//! lost). A growing queue raises a [`WhisperStatus`] lag signal (EXC-WHISPER-LAG).

pub mod model_mgr;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};
use uuid::Uuid;
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

use crate::audio::vad::{Segmenter, Utterance};
use crate::audio::{SampleChunk, StreamTag};
use crate::error::{AppError, AppResult};
use crate::session::model::TranscriptEntry;
use crate::storage;

/// Queued-chunk count above which the pipeline reports "lagging". Heuristic: at
/// the cpal callback cadence this is on the order of a second of backlog.
const LAG_THRESHOLD: usize = 100;

/// Lag signal for the UI (EXC-WHISPER-LAG).
#[derive(Debug, Clone, Copy)]
pub struct WhisperStatus {
    pub lagging: bool,
    pub queue_depth: usize,
}

/// Wiring for a pipeline run.
pub struct SttConfig {
    /// ggml model blob to load.
    pub model_path: PathBuf,
    /// Where to append finalized entries (the session's `transcript.jsonl`).
    pub transcript_path: PathBuf,
    /// Live forward of each finalized entry (PR3 emits `transcript-entry`).
    pub entry_tx: Sender<TranscriptEntry>,
    /// Lag status updates.
    pub status_tx: Sender<WhisperStatus>,
}

/// Handle to a running STT pipeline. Feed it via [`SttPipeline::sender`].
pub struct SttPipeline {
    chunk_tx: Sender<SampleChunk>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<AppResult<()>>>,
}

impl SttPipeline {
    /// Load the model and start the worker. The model is loaded **here** (on the
    /// caller's thread) so a missing/corrupt model surfaces synchronously; only
    /// the `Send` Whisper state moves into the worker.
    pub fn start(config: SttConfig) -> AppResult<Self> {
        let model_str = config
            .model_path
            .to_str()
            .ok_or_else(|| AppError::Model("model path is not valid UTF-8".into()))?;
        let ctx = WhisperContext::new_with_params(model_str, WhisperContextParameters::default())
            .map_err(|e| {
                AppError::Stt(format!("load model {}: {e:?}", config.model_path.display()))
            })?;
        // One reusable state; it holds an Arc to the context internals, so the
        // context wrapper can drop while the worker keeps transcribing.
        let state = ctx
            .create_state()
            .map_err(|e| AppError::Stt(format!("create whisper state: {e:?}")))?;

        let (chunk_tx, chunk_rx) = unbounded::<SampleChunk>();
        let stop = Arc::new(AtomicBool::new(false));
        let worker = {
            let stop = stop.clone();
            thread::Builder::new()
                .name("stt-whisper".into())
                .spawn(move || run_stt(state, chunk_rx, stop, config))
                .map_err(|e| AppError::Stt(format!("spawn stt thread: {e}")))?
        };

        Ok(Self {
            chunk_tx,
            stop,
            worker: Some(worker),
        })
    }

    /// A sender capture tees 16 kHz chunks into.
    pub fn sender(&self) -> Sender<SampleChunk> {
        self.chunk_tx.clone()
    }

    /// Stop: flush the segmenters, transcribe any final utterances, and join.
    pub fn stop(mut self) -> AppResult<()> {
        self.stop.store(true, Ordering::Relaxed);
        match self.worker.take() {
            Some(h) => h
                .join()
                .map_err(|_| AppError::Stt("stt worker panicked".into()))?,
            None => Ok(()),
        }
    }
}

impl Drop for SttPipeline {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
    }
}

fn run_stt(
    mut state: WhisperState,
    chunk_rx: Receiver<SampleChunk>,
    stop: Arc<AtomicBool>,
    config: SttConfig,
) -> AppResult<()> {
    let mut you_seg = Segmenter::new(StreamTag::You);
    let mut remote_seg = Segmenter::new(StreamTag::Remote);
    let mut utts: Vec<Utterance> = Vec::new();
    let mut lagging = false;

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match chunk_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => {
                segment(&mut you_seg, &mut remote_seg, &chunk, &mut utts);
                transcribe_pending(&mut state, &mut utts, &config)?;
                update_lag(&chunk_rx, &mut lagging, &config);
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    // Drain any buffered chunks, flush the segmenter tails, transcribe the rest.
    while let Ok(chunk) = chunk_rx.try_recv() {
        segment(&mut you_seg, &mut remote_seg, &chunk, &mut utts);
    }
    you_seg.finish(&mut utts);
    remote_seg.finish(&mut utts);
    transcribe_pending(&mut state, &mut utts, &config)?;
    Ok(())
}

fn segment(
    you_seg: &mut Segmenter,
    remote_seg: &mut Segmenter,
    chunk: &SampleChunk,
    utts: &mut Vec<Utterance>,
) {
    match chunk.tag {
        StreamTag::You => you_seg.push(&chunk.samples, utts),
        StreamTag::Remote => remote_seg.push(&chunk.samples, utts),
    }
}

/// Transcribe and persist every pending utterance, draining the buffer.
fn transcribe_pending(
    state: &mut WhisperState,
    utts: &mut Vec<Utterance>,
    config: &SttConfig,
) -> AppResult<()> {
    for utt in utts.drain(..) {
        let (text, confidence) = transcribe(state, &utt.samples)?;
        let text = text.trim().to_string();
        if text.is_empty() {
            continue; // whisper found no words (pure noise/silence got through VAD)
        }
        let entry = TranscriptEntry {
            id: Uuid::new_v4().to_string(),
            t_ms: utt.t_ms,
            stream: utt.tag,
            text,
            confidence,
        };
        // Ground-truth append first, then best-effort live forward.
        storage::append_transcript_entry_at(&config.transcript_path, &entry)?;
        let _ = config.entry_tx.send(entry);
    }
    Ok(())
}

/// Run Whisper on one utterance; return `(text, mean_token_probability)`.
fn transcribe(state: &mut WhisperState, samples: &[f32]) -> AppResult<(String, f32)> {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("en"));
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    state
        .full(params, samples)
        .map_err(|e| AppError::Stt(format!("transcription failed: {e:?}")))?;

    let n = state.full_n_segments();
    let mut text = String::new();
    let mut prob_sum = 0.0f64;
    let mut prob_count = 0u32;
    for i in 0..n {
        let seg = state
            .get_segment(i)
            .ok_or_else(|| AppError::Stt(format!("segment {i} out of bounds")))?;
        let s = seg
            .to_str_lossy()
            .map_err(|e| AppError::Stt(format!("read segment {i}: {e:?}")))?;
        text.push_str(s.as_ref());
        let n_tokens = seg.n_tokens();
        for j in 0..n_tokens {
            if let Some(tok) = seg.get_token(j) {
                prob_sum += tok.token_probability() as f64;
                prob_count += 1;
            }
        }
    }

    let confidence = if prob_count > 0 {
        (prob_sum / prob_count as f64) as f32
    } else {
        1.0
    };
    Ok((text, confidence))
}

/// Emit a lag status update when the queue crosses the threshold.
fn update_lag(chunk_rx: &Receiver<SampleChunk>, lagging: &mut bool, config: &SttConfig) {
    let depth = chunk_rx.len();
    let now = depth > LAG_THRESHOLD;
    if now != *lagging {
        *lagging = now;
        let _ = config.status_tx.send(WhisperStatus {
            lagging: now,
            queue_depth: depth,
        });
    }
}
