//! Dual-stream passive capture (technical-design.md §4).
//!
//! Two `cpal` input streams run in parallel — the real **mic → "you"** and the
//! **BlackHole/Call Assistant → "remote"** loopback. Each callback does only the
//! cheap work (sample-format convert + down-mix to mono) and hands raw chunks to
//! a single **audio-worker thread** over a `crossbeam` channel; the worker
//! resamples each side to 16 kHz with `rubato` and writes the interleaved stereo
//! ground-truth WAV (L = you, R = remote). No heavy processing on the real-time
//! audio callback, per rubato's streaming guidance.
//!
//! The cpal `Stream`s are `!Send`, so they are created, played, and dropped
//! entirely inside the worker thread; the public [`CaptureSession`] handle holds
//! only `Send` control signals + the join handle. In PR2 the worker also tees
//! the 16 kHz mono chunks to the VAD/Whisper feed.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Device, SampleFormat, StreamConfig};
use crossbeam_channel::{bounded, unbounded, RecvTimeoutError, Sender};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

use super::StreamTag;
use crate::audio::wav::{StereoWavWriter, SAMPLE_RATE};
use crate::error::{AppError, AppResult};

/// Flush the WAV (rewrite header + sync to disk) roughly once per second so a
/// crash loses at most ~1 s of tail (build principle #3; recovery in PR3).
const FLUSH_EVERY_FRAMES: u64 = SAMPLE_RATE as u64;

/// A down-mixed mono chunk at the device's native rate, tagged by side, sent
/// from a cpal callback to the audio worker.
struct RawChunk {
    tag: StreamTag,
    samples: Vec<f32>,
}

/// Result of a finished capture.
pub struct CaptureSummary {
    /// Stereo frames written to the WAV.
    pub frames_written: u64,
    /// Path of the finalized `audio.wav`.
    pub wav_path: PathBuf,
}

// ----------------------------------------------------------------------------
// Pure DSP (unit-tested without hardware)
// ----------------------------------------------------------------------------

/// Down-mix interleaved samples to mono by averaging each frame's channels.
/// `channels == 1` is a pass-through.
pub fn downmix_to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

/// Streaming resampler: buffers mono input at `in_rate` and emits 16 kHz mono.
///
/// Wraps rubato's synchronous FFT resampler (fixed-input mode: feed a fixed
/// chunk, read a variable amount out). The ratio is constant for a session, so
/// the FFT resampler is the fast, high-quality choice (README "fixed ratio").
pub struct StreamResampler {
    rs: Fft<f32>,
    inbuf: Vec<f32>,
}

impl StreamResampler {
    pub fn new(in_rate: u32, out_rate: u32) -> AppResult<Self> {
        // A power-of-two target chunk is a fine default (README "easy value").
        let chunk = 1024;
        let rs = Fft::<f32>::new(in_rate as usize, out_rate as usize, chunk, 2, 1, FixedSync::Input)
            .map_err(|e| AppError::Audio(format!("resampler init {in_rate}->{out_rate}: {e}")))?;
        Ok(Self {
            rs,
            inbuf: Vec::new(),
        })
    }

    /// Append `samples` and emit any resampled output into `out`.
    pub fn push(&mut self, samples: &[f32], out: &mut Vec<f32>) -> AppResult<()> {
        self.inbuf.extend_from_slice(samples);
        loop {
            let need = self.rs.input_frames_next();
            if self.inbuf.len() < need {
                break;
            }
            let chunk: Vec<f32> = self.inbuf.drain(..need).collect();
            self.run(&chunk, need, None, out)?;
        }
        Ok(())
    }

    /// Flush the final partial chunk (padding the tail with silence) so no
    /// captured audio is dropped at end-of-session.
    pub fn finish(&mut self, out: &mut Vec<f32>) -> AppResult<()> {
        let rem = self.inbuf.len();
        if rem == 0 {
            return Ok(());
        }
        let need = self.rs.input_frames_next();
        let mut chunk = std::mem::take(&mut self.inbuf);
        chunk.resize(need.max(rem), 0.0);
        let indexing = Indexing {
            input_offset: 0,
            output_offset: 0,
            active_channels_mask: None,
            partial_len: Some(rem),
        };
        self.run(&chunk[..need], need, Some(&indexing), out)
    }

    fn run(
        &mut self,
        input: &[f32],
        frames: usize,
        indexing: Option<&Indexing>,
        out: &mut Vec<f32>,
    ) -> AppResult<()> {
        let in_ad = InterleavedSlice::new(input, 1, frames)
            .map_err(|e| AppError::Audio(format!("resampler input adapter: {e:?}")))?;
        let out_max = self.rs.output_frames_max();
        let mut scratch = vec![0f32; out_max];
        let mut out_ad = InterleavedSlice::new_mut(&mut scratch, 1, out_max)
            .map_err(|e| AppError::Audio(format!("resampler output adapter: {e:?}")))?;
        let (_read, written) = self
            .rs
            .process_into_buffer(&in_ad, &mut out_ad, indexing)
            .map_err(|e| AppError::Audio(format!("resample: {e:?}")))?;
        out.extend_from_slice(&scratch[..written]);
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// cpal stream plumbing
// ----------------------------------------------------------------------------

/// Build (but do not start) an input stream that down-mixes to mono and forwards
/// `RawChunk`s to the worker. Returns the stream and its native sample rate.
/// While `paused` is set the callback drops its data (we are passive; the call
/// is unaffected).
fn build_input_stream(
    device: &Device,
    tag: StreamTag,
    tx: Sender<RawChunk>,
    paused: Arc<AtomicBool>,
) -> AppResult<(cpal::Stream, u32)> {
    let supported = device
        .default_input_config()
        .map_err(|e| AppError::Audio(format!("no default input config: {e}")))?;
    let sample_format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let in_channels = config.channels as usize;
    let in_rate = config.sample_rate.0;

    let err_fn = |e| eprintln!("[audio] stream error: {e}");

    macro_rules! make_stream {
        ($ty:ty, $conv:expr) => {{
            let tx = tx.clone();
            let paused = paused.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[$ty], _| {
                        if paused.load(Ordering::Relaxed) {
                            return;
                        }
                        let as_f32: Vec<f32> = data.iter().map(|&s| $conv(s)).collect();
                        let mono = downmix_to_mono(&as_f32, in_channels);
                        // Unbounded ground-truth path; send never blocks the callback.
                        let _ = tx.send(RawChunk { tag, samples: mono });
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AppError::Audio(format!("build_input_stream: {e}")))?
        }};
    }

    let stream = match sample_format {
        SampleFormat::F32 => make_stream!(f32, |s: f32| s),
        SampleFormat::I16 => make_stream!(i16, |s: i16| s as f32 / i16::MAX as f32),
        SampleFormat::U16 => make_stream!(u16, |s: u16| (s as f32 / u16::MAX as f32) * 2.0 - 1.0),
        other => {
            return Err(AppError::Audio(format!(
                "unsupported sample format on {tag:?} stream: {other:?}"
            )))
        }
    };

    Ok((stream, in_rate))
}

// ----------------------------------------------------------------------------
// CaptureSession — public handle
// ----------------------------------------------------------------------------

/// A live dual-stream capture. Owns the control signals + worker thread; the
/// cpal streams live inside the worker (they are `!Send`).
pub struct CaptureSession {
    paused: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<AppResult<CaptureSummary>>>,
}

impl CaptureSession {
    /// Open the mic + remote devices (by the ids from `list_audio_input_devices`)
    /// and begin writing `wav_path`. Returns once both streams are live, or an
    /// error if any device/stream/WAV setup failed.
    pub fn start(mic_id: String, remote_id: String, wav_path: PathBuf) -> AppResult<Self> {
        let paused = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = bounded::<AppResult<()>>(1);

        let worker = {
            let paused = paused.clone();
            let stop = stop.clone();
            thread::Builder::new()
                .name("audio-capture".into())
                .spawn(move || run_capture(mic_id, remote_id, wav_path, paused, stop, ready_tx))
                .map_err(|e| AppError::Audio(format!("spawn capture thread: {e}")))?
        };

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                paused,
                stop,
                worker: Some(worker),
            }),
            Ok(Err(e)) => {
                let _ = worker.join();
                Err(e)
            }
            Err(_) => {
                let _ = worker.join();
                Err(AppError::Audio("capture worker exited before signaling ready".into()))
            }
        }
    }

    /// Suspend capture (stops feeding WAV; the meeting is unaffected).
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
    }

    /// Resume capture after a pause.
    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
    }

    /// Whether capture is currently paused.
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    /// Stop capture: tear down the streams, flush + finalize the WAV, and return
    /// the summary. Consumes the handle.
    pub fn stop(mut self) -> AppResult<CaptureSummary> {
        self.stop.store(true, Ordering::Relaxed);
        match self.worker.take() {
            Some(h) => h
                .join()
                .map_err(|_| AppError::Audio("capture worker panicked".into()))?,
            None => Err(AppError::Audio("capture already stopped".into())),
        }
    }
}

impl Drop for CaptureSession {
    fn drop(&mut self) {
        // If stop() wasn't called (e.g. the handle is dropped on error), still
        // tear the worker down so the streams + WAV close cleanly.
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
    }
}

/// The worker thread: resolve devices, open streams + WAV, then resample and
/// write until `stop`. Errors during setup are reported via `ready_tx`.
fn run_capture(
    mic_id: String,
    remote_id: String,
    wav_path: PathBuf,
    paused: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    ready_tx: Sender<AppResult<()>>,
) -> AppResult<CaptureSummary> {
    // --- Setup: any failure here is reported to the caller via ready_tx. ---
    let setup = (|| -> AppResult<_> {
        let mic = super::find_input_device_by_id(&mic_id)?;
        let remote = super::find_input_device_by_id(&remote_id)?;
        let writer = StereoWavWriter::create(&wav_path)?;

        let (tx, rx) = unbounded::<RawChunk>();
        let (mic_stream, mic_rate) =
            build_input_stream(&mic, StreamTag::You, tx.clone(), paused.clone())?;
        let (remote_stream, remote_rate) =
            build_input_stream(&remote, StreamTag::Remote, tx.clone(), paused.clone())?;
        drop(tx); // only the streams hold senders now → rx disconnects when they drop

        let you_rs = StreamResampler::new(mic_rate, SAMPLE_RATE)?;
        let remote_rs = StreamResampler::new(remote_rate, SAMPLE_RATE)?;

        mic_stream
            .play()
            .map_err(|e| AppError::Audio(format!("mic play: {e}")))?;
        remote_stream
            .play()
            .map_err(|e| AppError::Audio(format!("remote play: {e}")))?;

        Ok((writer, rx, mic_stream, remote_stream, you_rs, remote_rs))
    })();

    let (mut writer, rx, mic_stream, remote_stream, mut you_rs, mut remote_rs) = match setup {
        Ok(v) => {
            let _ = ready_tx.send(Ok(()));
            v
        }
        Err(e) => {
            let _ = ready_tx.send(Err(e.clone()));
            return Err(e);
        }
    };

    let mut you_q: VecDeque<f32> = VecDeque::new();
    let mut remote_q: VecDeque<f32> = VecDeque::new();
    let mut scratch: Vec<f32> = Vec::new();
    let mut frames_since_flush: u64 = 0;

    // --- Main loop: resample + interleave + write until stop. ---
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => {
                process_chunk(&chunk, &mut you_rs, &mut remote_rs, &mut you_q, &mut remote_q, &mut scratch)?;
                frames_since_flush += drain_interleave(&mut writer, &mut you_q, &mut remote_q)?;
                if frames_since_flush >= FLUSH_EVERY_FRAMES {
                    writer.flush()?;
                    frames_since_flush = 0;
                }
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    // --- Teardown: stop callbacks, drain buffered audio, flush tails. ---
    drop(mic_stream);
    drop(remote_stream);
    while let Ok(chunk) = rx.try_recv() {
        process_chunk(&chunk, &mut you_rs, &mut remote_rs, &mut you_q, &mut remote_q, &mut scratch)?;
        drain_interleave(&mut writer, &mut you_q, &mut remote_q)?;
    }
    scratch.clear();
    you_rs.finish(&mut scratch)?;
    you_q.extend(scratch.drain(..));
    remote_rs.finish(&mut scratch)?;
    remote_q.extend(scratch.drain(..));
    drain_interleave(&mut writer, &mut you_q, &mut remote_q)?;
    flush_remainder(&mut writer, &mut you_q, &mut remote_q)?;

    let frames_written = writer.finalize()?;
    Ok(CaptureSummary {
        frames_written,
        wav_path,
    })
}

/// Resample one tagged chunk into its side's 16 kHz output queue.
fn process_chunk(
    chunk: &RawChunk,
    you_rs: &mut StreamResampler,
    remote_rs: &mut StreamResampler,
    you_q: &mut VecDeque<f32>,
    remote_q: &mut VecDeque<f32>,
    scratch: &mut Vec<f32>,
) -> AppResult<()> {
    scratch.clear();
    match chunk.tag {
        StreamTag::You => {
            you_rs.push(&chunk.samples, scratch)?;
            you_q.extend(scratch.drain(..));
        }
        StreamTag::Remote => {
            remote_rs.push(&chunk.samples, scratch)?;
            remote_q.extend(scratch.drain(..));
        }
    }
    Ok(())
}

/// Write as many fully-paired stereo frames as both queues currently hold.
/// Returns the number of frames written.
fn drain_interleave(
    writer: &mut StereoWavWriter,
    you_q: &mut VecDeque<f32>,
    remote_q: &mut VecDeque<f32>,
) -> AppResult<u64> {
    let n = you_q.len().min(remote_q.len());
    for _ in 0..n {
        let l = you_q.pop_front().unwrap();
        let r = remote_q.pop_front().unwrap();
        writer.write_frame(l, r)?;
    }
    Ok(n as u64)
}

/// Drain the unequal tail at end-of-session, padding the shorter side with
/// silence so every captured sample lands in the ground-truth WAV.
fn flush_remainder(
    writer: &mut StereoWavWriter,
    you_q: &mut VecDeque<f32>,
    remote_q: &mut VecDeque<f32>,
) -> AppResult<()> {
    let n = you_q.len().max(remote_q.len());
    for _ in 0..n {
        let l = you_q.pop_front().unwrap_or(0.0);
        let r = remote_q.pop_front().unwrap_or(0.0);
        writer.write_frame(l, r)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_averages_stereo() {
        // 3 stereo frames: (1,-1)->0, (0.5,0.5)->0.5, (0.2,0.0)->0.1
        let interleaved = [1.0, -1.0, 0.5, 0.5, 0.2, 0.0];
        let mono = downmix_to_mono(&interleaved, 2);
        assert_eq!(mono.len(), 3);
        assert!((mono[0] - 0.0).abs() < 1e-6);
        assert!((mono[1] - 0.5).abs() < 1e-6);
        assert!((mono[2] - 0.1).abs() < 1e-6);
    }

    #[test]
    fn downmix_mono_is_passthrough() {
        let x = [0.1, 0.2, 0.3];
        assert_eq!(downmix_to_mono(&x, 1), vec![0.1, 0.2, 0.3]);
    }

    fn resampled_len(in_rate: u32, secs: u32) -> usize {
        let mut rs = StreamResampler::new(in_rate, SAMPLE_RATE).unwrap();
        let mut out = Vec::new();
        let n = (in_rate * secs) as usize;
        // A 220 Hz-ish tone so the signal is non-trivial.
        let input: Vec<f32> = (0..n)
            .map(|i| (i as f32 * 0.03).sin() * 0.3)
            .collect();
        rs.push(&input, &mut out).unwrap();
        rs.finish(&mut out).unwrap();
        assert!(out.iter().all(|s| s.is_finite()), "resampler produced non-finite output");
        out.len()
    }

    #[test]
    fn resamples_48k_to_16k() {
        // 1 s of 48 kHz → ~16 kHz. Allow generous slack for FFT edge effects;
        // the point is it's clearly ~1/3, not a pass-through or a doubling.
        let n = resampled_len(48_000, 1) as i64;
        assert!((14_000..=18_000).contains(&n), "expected ~16000, got {n}");
    }

    #[test]
    fn resamples_44k1_to_16k() {
        let n = resampled_len(44_100, 1) as i64;
        assert!((14_000..=18_000).contains(&n), "expected ~16000, got {n}");
    }
}
