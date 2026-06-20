//! Manual on-device smoke test for the M2/PR2 STT pipeline:
//! capture/feed → VAD segmentation → Whisper → `TranscriptEntry`.
//!
//!   # transcribe a known 16 kHz mono WAV (deterministic — best for verifying):
//!   cargo run --release --example transcribe_smoke -- file clip.wav [model.bin]
//!
//!   # live: mic ("you") + BlackHole ("remote") for N seconds:
//!   cargo run --release --example transcribe_smoke -- live 20 [model.bin]
//!
//! With no model path, it uses the app's downloaded `small` model, falling back
//! to the M0 spike's `spikes/models/ggml-small.bin` if present. Build `--release`
//! (or rely on the dev opt-level for deps) so Whisper runs at a usable speed.

use std::path::PathBuf;
use std::thread::{self, sleep};
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver};

use call_assistant_lib::audio::{self, capture::CaptureSession, SampleChunk, StreamTag};
use call_assistant_lib::session::TranscriptEntry;
use call_assistant_lib::stt::{model_mgr, SttConfig, SttPipeline, WhisperStatus};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("file") => run_file(args.get(1), args.get(2)),
        Some("live") => run_live(args.get(1), args.get(2)),
        _ => run_live(args.first(), args.get(1)), // bare `[seconds] [model]` → live
    }
}

/// Resolve a model: explicit path, else the app's `small`, else the spike's.
fn resolve_model(arg: Option<&String>) -> PathBuf {
    if let Some(p) = arg {
        return PathBuf::from(p);
    }
    if let Ok(app) = model_mgr::model_path("small") {
        if model_mgr::is_valid_ggml(&app) {
            return app;
        }
    }
    for candidate in ["../spikes/models/ggml-small.bin", "spikes/models/ggml-small.bin"] {
        let p = PathBuf::from(candidate);
        if model_mgr::is_valid_ggml(&p) {
            return p;
        }
    }
    eprintln!(
        "No Whisper model found. Pass one explicitly, or fetch one:\n  \
         (cd spikes && ./fetch-model.sh small)"
    );
    std::process::exit(1);
}

/// Start the pipeline and a thread that prints entries + lag as they arrive.
/// Returns the pipeline and the printer's join handle (yields the entry count).
fn start_pipeline(model_path: PathBuf) -> (SttPipeline, thread::JoinHandle<usize>) {
    let transcript_path = std::env::temp_dir().join("callassistant_transcribe_smoke.jsonl");
    let _ = std::fs::remove_file(&transcript_path);

    let (entry_tx, entry_rx) = unbounded::<TranscriptEntry>();
    let (status_tx, status_rx) = unbounded::<WhisperStatus>();

    println!("loading model {} …", model_path.display());
    let pipeline = SttPipeline::start(SttConfig {
        model_path,
        transcript_path: transcript_path.clone(),
        entry_tx,
        status_tx,
    })
    .unwrap_or_else(|e| {
        eprintln!("failed to load model: {e}");
        std::process::exit(1);
    });
    println!("transcript → {}", transcript_path.display());

    let printer = spawn_printer(entry_rx, status_rx);
    (pipeline, printer)
}

fn spawn_printer(
    entry_rx: Receiver<TranscriptEntry>,
    status_rx: Receiver<WhisperStatus>,
) -> thread::JoinHandle<usize> {
    thread::spawn(move || {
        // Drain status on a side thread; it ends when the channel disconnects.
        let status = thread::spawn(move || {
            while let Ok(s) = status_rx.recv() {
                eprintln!(
                    "  (whisper {} — queue {})",
                    if s.lagging { "LAGGING" } else { "caught up" },
                    s.queue_depth
                );
            }
        });
        let mut count = 0usize;
        while let Ok(e) = entry_rx.recv() {
            count += 1;
            println!(
                "  {}  {:<6} ({:.2})  {}",
                fmt_ts(e.t_ms),
                format!("{:?}", e.stream).to_uppercase(),
                e.confidence,
                e.text
            );
        }
        let _ = status.join();
        count
    })
}

fn fmt_ts(ms: u64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}", s / 60, s % 60)
}

fn run_live(seconds_arg: Option<&String>, model_arg: Option<&String>) {
    let seconds: u64 = seconds_arg.and_then(|s| s.parse().ok()).unwrap_or(20);
    let model_path = resolve_model(model_arg);

    let devices = audio::list_input_devices().expect("enumerate inputs");
    let mic = devices
        .iter()
        .find(|d| d.is_default)
        .or_else(|| devices.first())
        .expect("no input devices");
    let remote_id = match audio::find_remote_loopback_id().expect("scan") {
        Some(id) => id,
        None => {
            eprintln!("No BlackHole/Call Assistant input found (see spikes/RUN-s3.md).");
            std::process::exit(1);
        }
    };
    println!("mic (L)={} | remote (R)={}", mic.name, remote_id);

    let (pipeline, printer) = start_pipeline(model_path);
    let wav = std::env::temp_dir().join("callassistant_transcribe_smoke.wav");
    let session =
        CaptureSession::start(mic.id.clone(), remote_id, wav.clone(), Some(pipeline.sender()), None)
            .unwrap_or_else(|e| {
                eprintln!("failed to start capture: {e}");
                std::process::exit(1);
            });

    println!("listening {seconds}s — speak into the mic / play audio through the Multi-Output…\n");
    sleep(Duration::from_secs(seconds));

    let summary = session.stop().expect("stop capture");
    pipeline.stop().expect("stop pipeline");
    let n = printer.join().unwrap();
    println!(
        "\ndone — {n} transcript entries; wav {} ({} frames)",
        summary.wav_path.display(),
        summary.frames_written
    );
}

fn run_file(wav_arg: Option<&String>, model_arg: Option<&String>) {
    let wav = wav_arg.unwrap_or_else(|| {
        eprintln!("usage: transcribe_smoke -- file <16k-mono.wav> [model.bin]");
        std::process::exit(1);
    });
    let samples = read_wav_16k_mono(wav).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    let model_path = resolve_model(model_arg);

    let (pipeline, printer) = start_pipeline(model_path);
    let tx = pipeline.sender();

    println!("feeding {:.1}s of audio through the pipeline…\n", samples.len() as f32 / 16_000.0);
    // Feed in ~0.5 s chunks (tagged "you") to mimic the live stream.
    for chunk in samples.chunks(8_000) {
        let _ = tx.send(SampleChunk {
            tag: StreamTag::You,
            samples: chunk.to_vec(),
        });
        sleep(Duration::from_millis(40));
    }
    drop(tx);
    pipeline.stop().expect("stop pipeline");
    let n = printer.join().unwrap();
    println!("\ndone — {n} transcript entries");
}

/// Read a 16 kHz WAV to mono f32 (down-mixing if needed). Errors if not 16 kHz.
fn read_wav_16k_mono(path: &str) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| format!("open WAV: {e}"))?;
    let spec = reader.spec();
    if spec.sample_rate != 16_000 {
        return Err(format!(
            "WAV is {} Hz; need 16000. Re-encode: ffmpeg -i in.wav -ar 16000 -ac 1 out.wav",
            spec.sample_rate
        ));
    }
    let channels = spec.channels.max(1) as usize;
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| format!("read float samples: {e}"))?,
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max))
                .collect::<Result<_, _>>()
                .map_err(|e| format!("read int samples: {e}"))?
        }
    };
    Ok(audio::capture::downmix_to_mono(&interleaved, channels))
}
