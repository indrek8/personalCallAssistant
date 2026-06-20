//! Energy-based voice-activity segmentation (technical-design.md §5).
//!
//! One [`Segmenter`] per stream (You / Remote) turns a continuous 16 kHz mono
//! feed into discrete [`Utterance`]s, cutting on a silence gap (~600 ms) with a
//! **hard-max length** (~12 s) so Whisper is fed regularly and we never wait
//! forever or slice mid-monologue. A short pre-roll is kept so word onsets
//! aren't clipped. The threshold tracks an adaptive noise floor, so it copes
//! with rooms at different levels rather than a single magic constant.
//!
//! This is intentionally simple, dependency-free, and deterministic (hence
//! unit-tested). The hard-max cut is the safety net even when the energy
//! heuristic is imperfect; webrtc-vad is a drop-in future swap behind the same
//! `Segmenter` interface.

use std::collections::VecDeque;

use super::StreamTag;

/// One analysis frame = 20 ms at 16 kHz.
const FRAME_SAMPLES: usize = 320;
const SAMPLE_RATE: u64 = 16_000;

/// Trailing silence that ends an utterance.
const SILENCE_HANG_MS: u64 = 600;
/// Force-cut a continuous utterance at this length (technical-design.md §5).
const MAX_UTTERANCE_MS: u64 = 12_000;
/// Drop utterances shorter than this (clicks, breaths, stray noise).
const MIN_UTTERANCE_MS: u64 = 200;
/// Audio kept before speech onset so the first word isn't clipped.
const PREROLL_MS: u64 = 200;

/// Absolute RMS floor: nothing below this counts as speech regardless of the
/// adaptive estimate (normalized f32 audio).
const ABS_RMS_FLOOR: f32 = 0.004;
/// Speech must exceed `noise_floor * SPEECH_FACTOR`.
const SPEECH_FACTOR: f32 = 3.0;

const fn ms_to_samples(ms: u64) -> usize {
    (ms * SAMPLE_RATE / 1000) as usize
}

fn samples_to_ms(s: u64) -> u64 {
    s * 1000 / SAMPLE_RATE
}

/// A segmented chunk of speech from one stream, ready for transcription.
#[derive(Debug, Clone)]
pub struct Utterance {
    pub tag: StreamTag,
    /// Start time from capture start, derived from the sample index (not wall
    /// clock), so it's stable regardless of transcription latency.
    pub t_ms: u64,
    /// 16 kHz mono speech samples.
    pub samples: Vec<f32>,
}

/// Per-stream voice-activity segmenter.
pub struct Segmenter {
    tag: StreamTag,
    noise_floor: f32,
    /// Recent samples kept while idle, prepended to the next utterance.
    preroll: VecDeque<f32>,
    /// Samples not yet forming a full frame.
    pending: Vec<f32>,
    /// Current (in-progress) utterance buffer, or empty when idle.
    cur: Vec<f32>,
    in_speech: bool,
    /// Trailing silence samples accumulated in the current utterance.
    trailing_silence: usize,
    /// Pre-roll samples prepended to the current utterance — excluded from the
    /// min-length gate so a brief blip plus pre-roll isn't mistaken for speech.
    cur_preroll_len: usize,
    /// Absolute sample index where the current utterance began.
    cur_start_sample: u64,
    /// Total samples consumed (for timestamps).
    total_samples: u64,
}

impl Segmenter {
    pub fn new(tag: StreamTag) -> Self {
        Self {
            tag,
            noise_floor: ABS_RMS_FLOOR,
            preroll: VecDeque::new(),
            pending: Vec::new(),
            cur: Vec::new(),
            in_speech: false,
            trailing_silence: 0,
            cur_preroll_len: 0,
            cur_start_sample: 0,
            total_samples: 0,
        }
    }

    /// Feed 16 kHz mono samples; append any completed utterances to `out`.
    pub fn push(&mut self, samples: &[f32], out: &mut Vec<Utterance>) {
        self.pending.extend_from_slice(samples);
        while self.pending.len() >= FRAME_SAMPLES {
            let frame: Vec<f32> = self.pending.drain(..FRAME_SAMPLES).collect();
            self.process_frame(&frame, out);
        }
    }

    /// Flush at end-of-session: emit the in-progress utterance (plus any pending
    /// tail) if it's long enough.
    pub fn finish(&mut self, out: &mut Vec<Utterance>) {
        if self.in_speech {
            let pending = std::mem::take(&mut self.pending);
            self.cur.extend_from_slice(&pending);
            self.emit(out);
        }
        self.reset_idle();
    }

    fn process_frame(&mut self, frame: &[f32], out: &mut Vec<Utterance>) {
        let frame_start = self.total_samples;
        let is_speech = self.classify(frame);
        self.total_samples += frame.len() as u64;

        if self.in_speech {
            self.cur.extend_from_slice(frame);
            if is_speech {
                self.trailing_silence = 0;
            } else {
                self.trailing_silence += frame.len();
            }

            if self.trailing_silence >= ms_to_samples(SILENCE_HANG_MS) {
                self.emit(out);
                self.reset_idle();
            } else if self.cur.len() >= ms_to_samples(MAX_UTTERANCE_MS) {
                // Hard-max cut: emit and immediately continue a new utterance
                // (no silence boundary, so no pre-roll).
                self.emit(out);
                self.cur.clear();
                self.cur_preroll_len = 0;
                self.trailing_silence = 0;
                self.cur_start_sample = self.total_samples;
                // stay in_speech
            }
        } else {
            if is_speech {
                let pre: Vec<f32> = self.preroll.drain(..).collect();
                self.cur_preroll_len = pre.len();
                self.cur_start_sample = frame_start.saturating_sub(pre.len() as u64);
                self.cur = pre;
                self.cur.extend_from_slice(frame);
                self.in_speech = true;
                self.trailing_silence = 0;
            } else {
                self.preroll.extend(frame.iter().copied());
                let max_pre = ms_to_samples(PREROLL_MS);
                while self.preroll.len() > max_pre {
                    self.preroll.pop_front();
                }
            }
        }
    }

    /// Classify a frame as speech. The noise floor is learned from **non-speech
    /// frames only**, so sustained speech never raises the threshold above its
    /// own level (which would make a long monologue read as silence).
    fn classify(&mut self, frame: &[f32]) -> bool {
        let rms = frame_rms(frame);
        let threshold = ABS_RMS_FLOOR.max(self.noise_floor * SPEECH_FACTOR);
        let is_speech = rms > threshold;
        if !is_speech {
            // Track toward the ambient level: down fast, up slow.
            if rms < self.noise_floor {
                self.noise_floor = 0.9 * self.noise_floor + 0.1 * rms;
            } else {
                self.noise_floor = 0.99 * self.noise_floor + 0.01 * rms;
            }
        }
        is_speech
    }

    /// Emit `cur` as an utterance if its **speech extent** (excluding pre-roll
    /// and trailing silence) clears the minimum length, trimming most of the
    /// trailing silence (keep a little so Whisper has a clean tail).
    fn emit(&mut self, out: &mut Vec<Utterance>) {
        let speech_content = self
            .cur
            .len()
            .saturating_sub(self.cur_preroll_len)
            .saturating_sub(self.trailing_silence);
        if samples_to_ms(speech_content as u64) < MIN_UTTERANCE_MS {
            return;
        }
        let keep_tail = ms_to_samples(100);
        let trim = self.trailing_silence.saturating_sub(keep_tail);
        let end = self.cur.len().saturating_sub(trim);
        out.push(Utterance {
            tag: self.tag,
            t_ms: samples_to_ms(self.cur_start_sample),
            samples: self.cur[..end].to_vec(),
        });
    }

    fn reset_idle(&mut self) {
        self.cur.clear();
        self.in_speech = false;
        self.trailing_silence = 0;
        self.preroll.clear();
    }
}

fn frame_rms(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = frame.iter().map(|s| s * s).sum();
    (sum_sq / frame.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn silence(secs: f32) -> Vec<f32> {
        vec![0.0; (secs * SAMPLE_RATE as f32) as usize]
    }

    fn tone(secs: f32, amp: f32) -> Vec<f32> {
        let n = (secs * SAMPLE_RATE as f32) as usize;
        (0..n)
            .map(|i| (i as f32 * 2.0 * std::f32::consts::PI * 220.0 / SAMPLE_RATE as f32).sin() * amp)
            .collect()
    }

    fn run(signal: &[f32]) -> Vec<Utterance> {
        let mut seg = Segmenter::new(StreamTag::You);
        let mut out = Vec::new();
        seg.push(signal, &mut out);
        seg.finish(&mut out);
        out
    }

    #[test]
    fn splits_two_utterances_on_silence() {
        let mut sig = Vec::new();
        sig.extend(silence(0.5));
        sig.extend(tone(1.0, 0.3));
        sig.extend(silence(1.0));
        sig.extend(tone(1.0, 0.3));
        sig.extend(silence(0.7));

        let utts = run(&sig);
        assert_eq!(utts.len(), 2, "expected two utterances, got {}", utts.len());
        assert!(utts[0].t_ms < utts[1].t_ms);
        // Each ~1 s of speech (+pre-roll/tail), well above the minimum.
        for u in &utts {
            let ms = samples_to_ms(u.samples.len() as u64);
            assert!((700..2000).contains(&ms), "utterance length {ms} ms out of range");
        }
        // First utterance starts near the 0.5 s onset (pre-roll pulls it earlier).
        assert!(utts[0].t_ms <= 600, "first onset {} ms too late", utts[0].t_ms);
    }

    #[test]
    fn hard_max_cuts_a_long_monologue() {
        // 15 s of unbroken speech must be cut (12 s cap) → at least two pieces.
        let utts = run(&tone(15.0, 0.3));
        assert!(utts.len() >= 2, "expected ≥2 pieces from a 15 s monologue, got {}", utts.len());
        let first = samples_to_ms(utts[0].samples.len() as u64);
        assert!((11_500..=12_500).contains(&first), "first cut at {first} ms, expected ~12 s");
    }

    #[test]
    fn drops_too_short_blips() {
        let mut sig = Vec::new();
        sig.extend(silence(0.4));
        sig.extend(tone(0.05, 0.3)); // 50 ms < 200 ms min
        sig.extend(silence(0.8));
        assert_eq!(run(&sig).len(), 0);
    }
}
