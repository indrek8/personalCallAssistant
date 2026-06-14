//! S2 · Concurrent Whisper ×2 spike.
//!
//! Validates the MVP's **2-stream** audio decision (architecture.md §4 /
//! build/milestones.md M0/S2): can we transcribe TWO streams (You + Remote) at
//! once and still keep up with realtime?
//!
//! It runs two independent whisper transcriptions concurrently — each its own
//! `WhisperContext` + state (nothing whisper-related is shared across threads),
//! both released together at a barrier so only the overlapping inference is
//! timed. It reports each stream's inference time and the **effective RTF**
//! (slowest stream ÷ audio duration):
//!   • RTF < 1.0  → two concurrent streams keep up with realtime → 2-stream model holds
//!   • RTF ≥ 1.0  → fall back to a single mixed-mono pass (generic "Speaker")
//!
//! Usage:
//!   cargo run --release --bin s2_concurrent_whisper -- <audio.wav> [model.bin]
//!
//! Use --release: whisper inference is far slower in a debug build and would
//! skew the timing. Compare the effective RTF against s1's single-stream RTF to
//! see the concurrency cost (on the Apple-Silicon GPU the two passes share the
//! Metal queue, so expect ~1.5–2× the single-stream number).

use std::path::Path;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

fn main() {
    if let Err(e) = run() {
        eprintln!("s2_concurrent_whisper error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);

    let wav_path = args.next().ok_or_else(|| {
        "missing WAV path.\n\
         usage: cargo run --release --bin s2_concurrent_whisper -- <audio.wav> [model.bin]"
            .to_string()
    })?;

    let model_path = args
        .next()
        .or_else(|| std::env::var("WHISPER_MODEL").ok())
        .unwrap_or_else(|| "models/ggml-small.bin".to_string());

    if !Path::new(&model_path).exists() {
        return Err(format!(
            "model not found at `{model_path}`.\n  Fetch one: ./fetch-model.sh small"
        ));
    }
    if !Path::new(&wav_path).exists() {
        return Err(format!("WAV not found at `{wav_path}`."));
    }

    let (samples, audio_secs) = read_wav_16k_mono(&wav_path)?;
    let samples = Arc::new(samples);
    println!("audio:   {wav_path}  ({audio_secs:.2}s @ 16kHz mono)");
    println!("model:   {model_path}");
    println!("streams: 2 concurrent (simulating You + Remote)\n");

    // Both streams release together at this barrier so the timed inference
    // windows genuinely overlap. Each thread owns its own context + state.
    let barrier = Arc::new(Barrier::new(2));
    let labels = ["you", "remote"];
    let mut handles = Vec::new();

    for label in labels {
        let model_path = model_path.clone();
        let samples = Arc::clone(&samples);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || -> Result<f64, String> {
            let ctx = WhisperContext::new_with_params(
                &model_path,
                WhisperContextParameters::default(),
            )
            .map_err(|e| format!("[{label}] failed to load model: {e:?}"))?;
            let mut state = ctx
                .create_state()
                .map_err(|e| format!("[{label}] failed to create state: {e:?}"))?;

            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_language(Some("en"));
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);

            // Setup done — release together so only concurrent inference is timed.
            barrier.wait();
            let t = Instant::now();
            state
                .full(params, &samples[..])
                .map_err(|e| format!("[{label}] transcription failed: {e:?}"))?;
            Ok(t.elapsed().as_secs_f64())
        }));
    }

    let mut durations = Vec::new();
    for h in handles {
        let d = h
            .join()
            .map_err(|_| "a stream thread panicked".to_string())??;
        durations.push(d);
    }

    let slowest = durations.iter().cloned().fold(0.0_f64, f64::max);
    let rtf = if audio_secs > 0.0 {
        slowest / audio_secs
    } else {
        f64::NAN
    };

    println!("--- timing (concurrent) ---");
    for (label, d) in labels.iter().zip(&durations) {
        println!("  {label:<7} inference: {d:.2}s");
    }
    println!("  audio duration:    {audio_secs:.2}s");
    println!(
        "  effective RTF:     {rtf:.3}  ({})",
        if rtf < 1.0 {
            "2 concurrent streams keep up with realtime — 2-stream model holds"
        } else {
            "2 concurrent streams fall behind — consider a single mixed-mono pass"
        }
    );

    Ok(())
}

/// Read a WAV and return `(samples, duration_seconds)` as f32 mono at 16 kHz.
/// (Same loader as s1: asserts 16 kHz, down-mixes multi-channel to mono.)
fn read_wav_16k_mono(path: &str) -> Result<(Vec<f32>, f64), String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("cannot open WAV: {e}"))?;
    let spec = reader.spec();

    if spec.sample_rate != 16_000 {
        return Err(format!(
            "WAV is {} Hz; Whisper needs 16000 Hz mono. Re-encode, e.g.:\n  \
             ffmpeg -i in.wav -ar 16000 -ac 1 -c:a pcm_s16le out.wav",
            spec.sample_rate
        ));
    }

    let channels = spec.channels.max(1) as usize;

    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| format!("failed to read float samples: {e}"))?,
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max))
                .collect::<Result<_, _>>()
                .map_err(|e| format!("failed to read int samples: {e}"))?
        }
    };

    let mono: Vec<f32> = if channels == 1 {
        interleaved
    } else {
        interleaved
            .chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    };

    let duration = mono.len() as f64 / 16_000.0;
    Ok((mono, duration))
}
