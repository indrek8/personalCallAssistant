# RUN — s2: Concurrent Whisper ×2

Validates the MVP's **2-stream** audio decision ([architecture.md §4](../docs/architecture.md) / [build/milestones.md](../docs/build/milestones.md) M0·S2): can we transcribe **two** streams (You + Remote) at the same time and still keep up with realtime?

## Prereqs
- A ggml model: `./fetch-model.sh small`
- A 16 kHz mono WAV (same as s1). Quick one via macOS `say`:
  ```
  say -o /tmp/test.wav --data-format=LEI16@16000 "About ten seconds of speech goes here so there is something for whisper to transcribe twice at once."
  ```

## Run
```
cargo run --release --bin s2_concurrent_whisper -- /tmp/test.wav
```
**Use `--release`** — whisper inference is far slower in a debug build, which would skew the timing.

## Reading the result
It runs two independent transcriptions concurrently (each its own context+state, released together so only the overlapping inference is timed) and prints each stream's inference time plus an **effective RTF** (slowest stream ÷ audio duration):

- **RTF < 1.0** → two concurrent streams keep up with realtime → the **2-stream You/Remote model holds**.
- **RTF ≥ 1.0** → concurrent passes fall behind → fall back to a single **mixed-mono** pass with a generic "Speaker" label.

Compare against **s1** (single-stream RTF) to see the concurrency cost. On the Apple-Silicon GPU (metal) both passes share the Metal queue, so expect the effective RTF to be roughly **1.5–2×** the single-stream number. If `small` is comfortably real-time solo (e.g. RTF ~0.3), two concurrent streams should still land under 1.0.
