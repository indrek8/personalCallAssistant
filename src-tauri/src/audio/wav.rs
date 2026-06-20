//! Incremental stereo WAV writer — the **ground-truth recording** (build
//! principle #3: the WAV is written incrementally so every failure degrades to
//! "you still have the recording").
//!
//! Layout: 16 kHz, stereo, 16-bit PCM, with **L = "you"** (mic) and
//! **R = "remote"** (BlackHole), matching technical-design.md §4/§9. The capture
//! worker calls [`StereoWavWriter::flush`] roughly once per second; `hound`'s
//! flush rewrites the RIFF/`data` chunk sizes and flushes to disk, so a crash
//! leaves a fully-valid WAV up to the last flush (at most ~1 s of tail loss).
//! [`repair_header`] is the recovery fallback (used by the boot crash scan in
//! PR3) for the rare crash between flushes.

use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::error::{AppError, AppResult};

/// Sample rate of the ground-truth WAV and the Whisper feed (technical-design.md §4).
pub const SAMPLE_RATE: u32 = 16_000;
/// Stereo: channel 0 = you, channel 1 = remote.
pub const CHANNELS: u16 = 2;
/// Bytes per interleaved stereo frame (2 channels × 16-bit).
const BYTES_PER_FRAME: u64 = (CHANNELS as u64) * 2;

fn spec() -> hound::WavSpec {
    hound::WavSpec {
        channels: CHANNELS,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    }
}

/// Convert a normalized `[-1.0, 1.0]` float sample to 16-bit PCM, clamping
/// out-of-range values rather than letting them wrap.
#[inline]
fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}

/// Incremental stereo WAV writer over a buffered file.
pub struct StereoWavWriter {
    inner: hound::WavWriter<std::io::BufWriter<fs::File>>,
    frames_written: u64,
}

impl StereoWavWriter {
    /// Create `path` and write the WAV header. Overwrites any existing file.
    pub fn create(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let inner = hound::WavWriter::create(path, spec())
            .map_err(|e| AppError::Audio(format!("cannot create WAV {}: {e}", path.display())))?;
        Ok(Self {
            inner,
            frames_written: 0,
        })
    }

    /// Append one stereo frame (`you` → L, `remote` → R).
    pub fn write_frame(&mut self, you: f32, remote: f32) -> AppResult<()> {
        self.inner
            .write_sample(to_i16(you))
            .map_err(|e| AppError::Audio(format!("WAV write (L): {e}")))?;
        self.inner
            .write_sample(to_i16(remote))
            .map_err(|e| AppError::Audio(format!("WAV write (R): {e}")))?;
        self.frames_written += 1;
        Ok(())
    }

    /// Frames (stereo sample pairs) written so far.
    pub fn frames_written(&self) -> u64 {
        self.frames_written
    }

    /// Flush buffered samples to disk and rewrite the header sizes. Called
    /// periodically by the capture worker so a crash loses at most the tail
    /// since the last flush.
    pub fn flush(&mut self) -> AppResult<()> {
        self.inner
            .flush()
            .map_err(|e| AppError::Audio(format!("WAV flush: {e}")))
    }

    /// Finalize: rewrite the header with the final length and flush. Consumes
    /// the writer. Returns the number of frames written.
    pub fn finalize(self) -> AppResult<u64> {
        let frames = self.frames_written;
        self.inner
            .finalize()
            .map_err(|e| AppError::Audio(format!("WAV finalize: {e}")))?;
        Ok(frames)
    }
}

/// Repair the RIFF / `data` chunk sizes of a WAV whose writer never finalized
/// (e.g. the app was killed mid-recording). `hound` patches these sizes on
/// flush/finalize; if neither ran since the last samples, the header
/// under-reports the data on disk. This rewrites both size fields from the
/// actual file length so the file is fully readable again, and returns the
/// number of stereo frames recovered.
///
/// The `fmt ` chunk written at creation is intact (its size is fixed), so we can
/// safely walk the chunk list to find `data` and assume it runs to EOF — true
/// for our single-pass, single-`data`-chunk writer.
pub fn repair_header(path: &Path) -> AppResult<u64> {
    let mut f = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| AppError::Audio(format!("cannot open WAV for repair {}: {e}", path.display())))?;
    let len = f.metadata()?.len();
    if len < 44 {
        return Err(AppError::Audio(format!(
            "WAV too short to repair ({} bytes): {}",
            len,
            path.display()
        )));
    }

    let mut riff = [0u8; 12];
    f.seek(SeekFrom::Start(0))?;
    f.read_exact(&mut riff)?;
    if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
        return Err(AppError::Audio(format!(
            "not a RIFF/WAVE file, cannot repair: {}",
            path.display()
        )));
    }

    // RIFF chunk size = whole file minus the 8-byte "RIFF<size>" prefix.
    f.seek(SeekFrom::Start(4))?;
    f.write_all(&((len - 8) as u32).to_le_bytes())?;

    // Walk chunks from offset 12 to find "data"; its payload runs to EOF.
    let mut pos: u64 = 12;
    loop {
        if pos + 8 > len {
            return Err(AppError::Audio(format!(
                "no data chunk found while repairing {}",
                path.display()
            )));
        }
        let mut hdr = [0u8; 8];
        f.seek(SeekFrom::Start(pos))?;
        f.read_exact(&mut hdr)?;
        let chunk_size = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as u64;

        if &hdr[0..4] == b"data" {
            let payload_start = pos + 8;
            let data_size = len - payload_start;
            f.seek(SeekFrom::Start(pos + 4))?;
            f.write_all(&(data_size as u32).to_le_bytes())?;
            f.flush()?;
            return Ok(data_size / BYTES_PER_FRAME);
        }

        // Skip a non-data chunk (e.g. `fmt `, whose size is valid). Chunks are
        // word-aligned, so round an odd size up by one padding byte.
        pos = pos + 8 + chunk_size + (chunk_size & 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("callassistant_wavtest_{}_{}.wav", std::process::id(), name))
    }

    #[test]
    fn round_trips_stereo_frames() {
        let path = tmp_path("roundtrip");
        let frames: Vec<(f32, f32)> = vec![(0.0, 0.0), (0.5, -0.5), (1.0, -1.0), (-0.25, 0.75)];

        let mut w = StereoWavWriter::create(&path).unwrap();
        for &(l, r) in &frames {
            w.write_frame(l, r).unwrap();
        }
        assert_eq!(w.frames_written(), frames.len() as u64);
        let n = w.finalize().unwrap();
        assert_eq!(n, frames.len() as u64);

        let mut reader = hound::WavReader::open(&path).unwrap();
        let s = reader.spec();
        assert_eq!(s.channels, CHANNELS);
        assert_eq!(s.sample_rate, SAMPLE_RATE);
        assert_eq!(s.bits_per_sample, 16);

        let samples: Vec<i16> = reader.samples::<i16>().map(|x| x.unwrap()).collect();
        assert_eq!(samples.len(), frames.len() * 2);
        for (i, &(l, r)) in frames.iter().enumerate() {
            assert_eq!(samples[i * 2], to_i16(l), "L mismatch at frame {i}");
            assert_eq!(samples[i * 2 + 1], to_i16(r), "R mismatch at frame {i}");
        }
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn repairs_a_header_with_zeroed_sizes() {
        let path = tmp_path("repair");
        let n_frames = 1000u64;

        let mut w = StereoWavWriter::create(&path).unwrap();
        for i in 0..n_frames {
            let v = (i as f32 / n_frames as f32) - 0.5;
            w.write_frame(v, -v).unwrap();
        }
        w.finalize().unwrap();

        // Simulate a crash that never wrote the final sizes: zero the RIFF size
        // (offset 4) and the data size (offset 40 in hound's canonical PCM header).
        {
            let mut f = OpenOptions::new().write(true).open(&path).unwrap();
            f.seek(SeekFrom::Start(4)).unwrap();
            f.write_all(&0u32.to_le_bytes()).unwrap();
            f.seek(SeekFrom::Start(40)).unwrap();
            f.write_all(&0u32.to_le_bytes()).unwrap();
        }

        // A reader should now see zero samples (broken header)…
        {
            let reader = hound::WavReader::open(&path).unwrap();
            assert_eq!(reader.len(), 0, "precondition: header reports no samples");
        }

        // …repair recovers every frame.
        let recovered = repair_header(&path).unwrap();
        assert_eq!(recovered, n_frames);

        let mut reader = hound::WavReader::open(&path).unwrap();
        let samples: Vec<i16> = reader.samples::<i16>().map(|x| x.unwrap()).collect();
        assert_eq!(samples.len() as u64, n_frames * 2);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn repair_rejects_malformed_files() {
        let dir = std::env::temp_dir();
        let small = dir.join(format!("ca_wav_small_{}.wav", std::process::id()));
        fs::write(&small, b"tiny").unwrap(); // < 44 bytes
        assert!(repair_header(&small).is_err());

        let not_riff = dir.join(format!("ca_wav_notriff_{}.wav", std::process::id()));
        fs::write(&not_riff, vec![0u8; 64]).unwrap(); // big enough, but no RIFF/WAVE
        assert!(repair_header(&not_riff).is_err());

        let _ = fs::remove_file(&small);
        let _ = fs::remove_file(&not_riff);
    }

    #[test]
    fn to_i16_clamps_without_wrapping() {
        assert_eq!(to_i16(0.0), 0);
        assert_eq!(to_i16(1.0), i16::MAX);
        assert_eq!(to_i16(-1.0), -i16::MAX); // −32767, symmetric — not i16::MIN
        // Inter-sample overshoot from the resampler must clamp, never wrap.
        assert_eq!(to_i16(2.0), i16::MAX);
        assert_eq!(to_i16(-2.0), -i16::MAX);
        assert_eq!(to_i16(f32::INFINITY), i16::MAX);
        assert_eq!(to_i16(f32::NEG_INFINITY), -i16::MAX);
    }

    #[test]
    fn repairs_unfinalized_file_with_post_flush_tail() {
        // The real EXC-CRASH shape: hound flushed N frames (header knows them), then
        // more frames were captured before the kill and never folded into the header.
        let path = tmp_path("repair_tail");
        let flushed = 50u64;
        {
            let mut w = StereoWavWriter::create(&path).unwrap();
            for i in 0..flushed {
                let v = i as f32 / 100.0;
                w.write_frame(v, -v).unwrap();
            }
            w.flush().unwrap(); // header now reports exactly `flushed` frames
        } // dropped without finalize()

        // Frames captured after the last flush: raw PCM appended past the header's
        // known length — exactly what a crash between flushes leaves on disk.
        let tail = 30u64;
        {
            let mut f = OpenOptions::new().append(true).open(&path).unwrap();
            for _ in 0..tail {
                f.write_all(&0i16.to_le_bytes()).unwrap(); // L
                f.write_all(&0i16.to_le_bytes()).unwrap(); // R
            }
        }

        // repair recomputes the data size from the real file length → every frame.
        assert_eq!(repair_header(&path).unwrap(), flushed + tail);
        let mut reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.samples::<i16>().count() as u64, (flushed + tail) * 2);
        let _ = fs::remove_file(&path);
    }
}
