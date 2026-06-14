# S1 · Whisper speed — RUN

**Goal:** prove `small` (and `base`) transcribe a 10 s 16 kHz mono WAV comfortably
faster than realtime (RTF < 1.0) on the target Mac. Decides `small` vs `base`.

## 1. Native build dep

`whisper-rs` builds whisper.cpp via `cmake`. If the first build fails with a
cmake error:

```sh
brew install cmake
```

(The `metal` feature in `Cargo.toml` uses the Apple-Silicon GPU automatically.)

## 2. Fetch a model

Use the helper (downloads from the official whisper.cpp Hugging Face repo into
`./models/`):

```sh
./fetch-model.sh small      # ~466 MB  (the default the spike looks for)
./fetch-model.sh base       # ~142 MB  (the fallback if small is too slow)
```

Or download manually:

```sh
mkdir -p models
curl -L https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin \
  -o models/ggml-small.bin
```

## 3. Get a 10 s, 16 kHz, mono WAV

The spike requires **16 kHz mono** (it errors with the re-encode command if not).
Any of these work:

**Record 10 s from your mic** (needs ffmpeg — `brew install ffmpeg`):

```sh
ffmpeg -f avfoundation -i ":0" -t 10 -ar 16000 -ac 1 -c:a pcm_s16le sample.wav
```

**Or re-encode an existing audio file to the right format:**

```sh
ffmpeg -i any-audio.mp3 -t 10 -ar 16000 -ac 1 -c:a pcm_s16le sample.wav
```

**Or grab the bundled whisper.cpp sample** (`jfk.wav` is already 16 kHz mono):

```sh
curl -L https://github.com/ggerganov/whisper.cpp/raw/master/samples/jfk.wav \
  -o sample.wav
```

## 4. Run

```sh
# uses models/ggml-small.bin by default
cargo run --release --bin s1_whisper -- sample.wav

# or pick a model explicitly
cargo run --release --bin s1_whisper -- sample.wav models/ggml-base.bin
```

> `--release` matters for realistic timing. (The dev profile already builds deps
> with optimization, but use `--release` for the headline RTF number.)

## 5. Read the result

The spike prints the transcript, then:

```
transcribe wall-clock: <s>
audio duration:        <s>
real-time factor (RTF): <ratio>  (faster than realtime — PASS for this model)
```

**Decision gate:** RTF for `small` comfortably < 1.0 → use `small`. Otherwise
fall back to `base` (re-run with `models/ggml-base.bin`).
