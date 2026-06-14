//! S3 · Dual-audio capture spike.
//!
//! Opens the default input mic AND a BlackHole input device at the same time
//! via `cpal`, captures ~10 s from both concurrently, and writes a stereo WAV
//! (L = mic, R = BlackHole) via `hound`. Prints which devices were used.
//!
//! This proves the §4 audio model: two independent physical streams give us
//! free 2-way speaker attribution (you = mic = L, remote = BlackHole = R) with
//! no diarization.
//!
//! Usage:
//!   cargo run --bin s3_dual_audio -- [seconds] [out.wav]
//!
//! Defaults: 10 seconds, `dual_capture.wav`.
//!
//! Prerequisites (see RUN.md): BlackHole installed, a Multi-Output Device
//! configured so remote audio reaches BlackHole, and a REBOOT after installing
//! BlackHole.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, StreamConfig};

const TARGET_RATE: u32 = 16_000; // we write a 16 kHz stereo file (§4)

fn main() {
    if let Err(e) = run() {
        eprintln!("s3_dual_audio error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let seconds: f64 = args
        .next()
        .map(|s| s.parse().unwrap_or(10.0))
        .unwrap_or(10.0);
    let out_path = args.next().unwrap_or_else(|| "dual_capture.wav".to_string());

    let host = cpal::default_host();

    // --- Pick the two devices. -------------------------------------------
    let mic = host
        .default_input_device()
        .ok_or_else(|| "no default input device found".to_string())?;
    let mic_name = mic.name().unwrap_or_else(|_| "<unknown>".into());

    let blackhole = find_blackhole(&host)
        .ok_or_else(|| {
            "no BlackHole input device found. Install BlackHole and REBOOT \
             (see RUN.md). Available inputs:\n"
                .to_string()
                + &list_input_devices(&host)
        })?;
    let blackhole_name = blackhole.name().unwrap_or_else(|_| "<unknown>".into());

    println!("mic  (L): {mic_name}");
    println!("black(R): {blackhole_name}");
    println!("capturing {seconds:.0}s...");

    // Shared mono buffers, each resampled to TARGET_RATE by the callbacks.
    let mic_buf = Arc::new(Mutex::new(Vec::<f32>::new()));
    let bh_buf = Arc::new(Mutex::new(Vec::<f32>::new()));
    let recording = Arc::new(AtomicBool::new(true));

    let mic_stream = build_input_stream(&mic, mic_buf.clone(), recording.clone())
        .map_err(|e| format!("failed to open mic stream: {e}"))?;
    let bh_stream =
        build_input_stream(&blackhole, bh_buf.clone(), recording.clone())
            .map_err(|e| format!("failed to open BlackHole stream: {e}"))?;

    // --- Run both streams concurrently. ----------------------------------
    let start = Instant::now();
    mic_stream
        .play()
        .map_err(|e| format!("mic play failed: {e}"))?;
    bh_stream
        .play()
        .map_err(|e| format!("BlackHole play failed: {e}"))?;

    while start.elapsed() < Duration::from_secs_f64(seconds) {
        std::thread::sleep(Duration::from_millis(100));
    }
    recording.store(false, Ordering::SeqCst);
    drop(mic_stream);
    drop(bh_stream);

    // --- Interleave into a stereo WAV. -----------------------------------
    let mic_samples = Arc::try_unwrap(mic_buf)
        .map(|m| m.into_inner().unwrap())
        .unwrap_or_default();
    let bh_samples = Arc::try_unwrap(bh_buf)
        .map(|m| m.into_inner().unwrap())
        .unwrap_or_default();

    let frames = mic_samples.len().max(bh_samples.len());
    println!(
        "captured: mic={} samples, blackhole={} samples ({} frames @ {}Hz)",
        mic_samples.len(),
        bh_samples.len(),
        frames,
        TARGET_RATE
    );

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: TARGET_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&out_path, spec)
        .map_err(|e| format!("cannot create WAV `{out_path}`: {e}"))?;

    for i in 0..frames {
        let l = mic_samples.get(i).copied().unwrap_or(0.0);
        let r = bh_samples.get(i).copied().unwrap_or(0.0);
        writer
            .write_sample(to_i16(l))
            .map_err(|e| format!("write L: {e}"))?;
        writer
            .write_sample(to_i16(r))
            .map_err(|e| format!("write R: {e}"))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("finalize WAV: {e}"))?;

    println!("\nwrote stereo WAV: {out_path}  (L=mic/you, R=blackhole/remote)");
    println!(
        "verify: play it back — your voice should be on the left, the remote \
         (e.g. a YouTube tab routed through the Multi-Output device) on the right."
    );

    Ok(())
}

/// Find a cpal input device whose name contains "blackhole" (case-insensitive).
fn find_blackhole(host: &cpal::Host) -> Option<Device> {
    host.input_devices().ok()?.find(|d| {
        d.name()
            .map(|n| n.to_lowercase().contains("blackhole"))
            .unwrap_or(false)
    })
}

fn list_input_devices(host: &cpal::Host) -> String {
    match host.input_devices() {
        Ok(devs) => devs
            .map(|d| format!("  - {}", d.name().unwrap_or_else(|_| "<unknown>".into())))
            .collect::<Vec<_>>()
            .join("\n"),
        Err(e) => format!("  (could not enumerate input devices: {e})"),
    }
}

/// Build an input stream that down-mixes to mono and naively decimates to
/// TARGET_RATE (nearest-sample), appending into `buf`. This is intentionally
/// crude — the spike only needs to prove simultaneous capture + attribution;
/// the real pipeline (M2) resamples properly with `rubato`.
fn build_input_stream(
    device: &Device,
    buf: Arc<Mutex<Vec<f32>>>,
    recording: Arc<AtomicBool>,
) -> Result<cpal::Stream, String> {
    let supported = device
        .default_input_config()
        .map_err(|e| format!("no default input config: {e}"))?;
    let sample_format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let in_channels = config.channels as usize;
    let in_rate = config.sample_rate.0;
    // Keep every Nth frame to approximate TARGET_RATE.
    let stride = (in_rate as f32 / TARGET_RATE as f32).max(1.0);

    let err_fn = |e| eprintln!("stream error: {e}");

    // Per-stream fractional-decimation accumulator.
    let acc = Arc::new(Mutex::new(0.0f32));

    macro_rules! make_stream {
        ($ty:ty, $to_f32:expr) => {{
            let buf = buf.clone();
            let recording = recording.clone();
            let acc = acc.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[$ty], _| {
                        if !recording.load(Ordering::SeqCst) {
                            return;
                        }
                        let mut out = buf.lock().unwrap();
                        let mut a = acc.lock().unwrap();
                        for frame in data.chunks(in_channels) {
                            // Down-mix to mono.
                            let mono = frame
                                .iter()
                                .map(|&s| $to_f32(s))
                                .sum::<f32>()
                                / in_channels as f32;
                            *a += 1.0;
                            if *a >= stride {
                                *a -= stride;
                                out.push(mono);
                            }
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("build_input_stream: {e}"))?
        }};
    }

    let stream = match sample_format {
        SampleFormat::F32 => make_stream!(f32, |s: f32| s),
        SampleFormat::I16 => make_stream!(i16, |s: i16| s as f32 / i16::MAX as f32),
        SampleFormat::U16 => {
            make_stream!(u16, |s: u16| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
        }
        other => return Err(format!("unsupported sample format: {other:?}")),
    };

    Ok(stream)
}

fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

