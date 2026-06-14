//! S1 · Whisper speed spike.
//!
//! Loads a ggml whisper model via the `whisper-rs` crate, transcribes a
//! 16 kHz mono WAV passed on the CLI, and prints the text plus wall-clock time
//! and real-time factor (RTF = processing_time / audio_duration). RTF < 1.0
//! means "transcribes faster than realtime" — the M0 acceptance bar for `small`.
//!
//! Usage:
//!   cargo run --bin s1_whisper -- <path-to-audio.wav> [path-to-model.bin]
//!
//! The model path defaults to `models/ggml-small.bin` (relative to the spikes
//! dir) or the `WHISPER_MODEL` env var. See RUN.md for how to fetch a model and
//! a sample WAV.

use std::path::Path;
use std::time::Instant;

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("s1_whisper error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);

    let wav_path = args.next().ok_or_else(|| {
        "missing WAV path.\n\
         usage: cargo run --bin s1_whisper -- <audio.wav> [model.bin]"
            .to_string()
    })?;

    let model_path = args
        .next()
        .or_else(|| std::env::var("WHISPER_MODEL").ok())
        .unwrap_or_else(|| "models/ggml-small.bin".to_string());

    if !Path::new(&model_path).exists() {
        return Err(format!(
            "model not found at `{model_path}`.\n\
             Fetch one (see RUN.md), e.g.:\n  \
             ./fetch-model.sh small\n\
             or pass an explicit path as the 2nd argument."
        ));
    }
    if !Path::new(&wav_path).exists() {
        return Err(format!("WAV not found at `{wav_path}`."));
    }

    // --- Load the 16 kHz mono samples from the WAV. -----------------------
    let (samples, audio_secs) = read_wav_16k_mono(&wav_path)?;
    println!("audio:  {wav_path}");
    println!("        {:.2}s @ 16kHz mono ({} samples)", audio_secs, samples.len());
    println!("model:  {model_path}");

    // --- Load the model. --------------------------------------------------
    let load_start = Instant::now();
    let ctx = WhisperContext::new_with_params(
        &model_path,
        WhisperContextParameters::default(),
    )
    .map_err(|e| format!("failed to load model: {e:?}"))?;
    let load_secs = load_start.elapsed().as_secs_f64();
    println!("load:   {load_secs:.2}s");

    // --- Transcribe. ------------------------------------------------------
    let mut state = ctx
        .create_state()
        .map_err(|e| format!("failed to create state: {e:?}"))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("en"));
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    let stt_start = Instant::now();
    state
        .full(params, &samples)
        .map_err(|e| format!("transcription failed: {e:?}"))?;
    let stt_secs = stt_start.elapsed().as_secs_f64();

    // --- Collect the text. ------------------------------------------------
    let n_segments = state.full_n_segments();
    let mut text = String::new();
    for i in 0..n_segments {
        let seg = state
            .get_segment(i)
            .ok_or_else(|| format!("segment {i} out of bounds"))?;
        let seg_text = seg
            .to_str_lossy()
            .map_err(|e| format!("failed to read segment {i}: {e:?}"))?;
        text.push_str(&seg_text);
    }

    let rtf = if audio_secs > 0.0 {
        stt_secs / audio_secs
    } else {
        f64::NAN
    };

    println!("\n--- transcript ---\n{}", text.trim());
    println!("\n--- timing ---");
    println!("transcribe wall-clock: {stt_secs:.2}s");
    println!("audio duration:        {audio_secs:.2}s");
    println!(
        "real-time factor (RTF): {rtf:.3}  ({})",
        if rtf < 1.0 {
            "faster than realtime — PASS for this model"
        } else {
            "slower than realtime — consider a smaller model"
        }
    );

    Ok(())
}

/// Read a WAV file and return `(samples, duration_seconds)` as f32 mono at
/// 16 kHz. Whisper requires exactly 16 kHz mono f32 input. This spike asserts
/// the file is already 16 kHz (the M2 pipeline will resample with `rubato`);
/// it does down-mix multi-channel input to mono so a stereo test file still
/// works.
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

    // Decode to f32 in [-1.0, 1.0] regardless of int/float storage.
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

    // Down-mix to mono by averaging channels.
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

