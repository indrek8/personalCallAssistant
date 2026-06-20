//! Manual on-device smoke test for the M2/PR1 capture → WAV pipeline.
//!
//! Opens the default mic ("you" → L) and the BlackHole/Call Assistant loopback
//! ("remote" → R), records for a few seconds (exercising pause/resume), writes a
//! 16 kHz stereo WAV, and reads it back to confirm it is valid. This is the
//! hardware counterpart to the pure-DSP unit tests — the cpal streams can't run
//! in CI, so run this locally to verify real capture:
//!
//!   cargo run --example capture_smoke                 # 8 s → /tmp/callassistant_capture.wav
//!   cargo run --example capture_smoke -- 12 out.wav   # custom duration + path
//!
//! Prereqs (same as M0/S3, see spikes/RUN-s3.md): BlackHole installed + a
//! Multi-Output Device routing remote audio to it, and mic permission granted.
//! While it records: talk into the mic (→ left) and play e.g. a YouTube tab
//! routed through the Multi-Output device (→ right).

use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use call_assistant_lib::audio::{self, capture::CaptureSession};

fn main() {
    let mut args = std::env::args().skip(1);
    let seconds: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(8);
    let out = args
        .next()
        .unwrap_or_else(|| "/tmp/callassistant_capture.wav".to_string());
    let out_path = PathBuf::from(&out);

    let devices = audio::list_input_devices().expect("enumerate input devices");
    let mic = devices
        .iter()
        .find(|d| d.is_default)
        .or_else(|| devices.first())
        .expect("no input devices available");
    let mic_id = mic.id.clone();

    let remote_id = match audio::find_remote_loopback_id().expect("scan for loopback") {
        Some(id) => id,
        None => {
            eprintln!(
                "No BlackHole / Call Assistant input found. Install BlackHole and reboot \
                 (see spikes/RUN-s3.md). Inputs cpal can see:"
            );
            for d in &devices {
                eprintln!("  - {}", d.name);
            }
            std::process::exit(1);
        }
    };

    println!("mic    (L): {}", mic.name);
    println!("remote (R): {remote_id}");
    println!("capturing {seconds}s → {out}");

    let session = match CaptureSession::start(mic_id, remote_id, out_path.clone(), None, None) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to start capture: {e}");
            eprintln!("(if this is a permissions error, grant mic access to your terminal/IDE)");
            std::process::exit(1);
        }
    };

    let half = seconds.max(2) / 2;
    sleep(Duration::from_secs(half));
    println!("… pause 1s (capture should drop this second)");
    session.pause();
    sleep(Duration::from_secs(1));
    session.resume();
    println!("… resumed");
    sleep(Duration::from_secs(half));

    let summary = session.stop().expect("stop capture");
    println!(
        "wrote {} frames (~{:.1}s @ 16 kHz) → {}",
        summary.frames_written,
        summary.frames_written as f64 / 16_000.0,
        summary.wav_path.display()
    );

    // Read it back to prove the file is a valid, finalized WAV.
    let reader = hound::WavReader::open(&out_path).expect("reopen WAV");
    let spec = reader.spec();
    println!(
        "verify: {} ch, {} Hz, {}-bit, {} frames — play it back, your voice on L, remote on R",
        spec.channels,
        spec.sample_rate,
        spec.bits_per_sample,
        reader.len() / spec.channels as u32
    );
}
