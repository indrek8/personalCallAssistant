# Whisper models (local — not committed)

This folder holds the **ggml Whisper model blobs** used by the M0 spikes (and, later, the app's `model_mgr`). The `.bin` files are large (142 MB – 1.4 GB) and are **gitignored** — only this reminder is committed. Re-fetch them whenever you need them.

## Fetch

From the `spikes/` directory:

```bash
./fetch-model.sh medium     # or: base | small   (default: small)
```

Downloads `ggml-<name>.bin` into this folder from the official whisper.cpp Hugging Face repo.

## Models

| Model | File | Size | Notes |
|-------|------|------|-------|
| base   | `ggml-base.bin`   | ~142 MB | fastest, lowest accuracy |
| small  | `ggml-small.bin`  | ~466 MB | fast fallback |
| medium | `ggml-medium.bin` | ~1.4 GB | **app default** — best accuracy, still ~18× realtime (M0/S1: RTF 0.055) |

URL pattern: `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-<name>.bin`

## Flaky network? Resilient (resume + retry) download

The 1.4 GB `medium` pull can hit `Connection reset by peer`. Resume in chunks until complete — run from `spikes/`:

```bash
URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin"
OUT="models/ggml-medium.bin"
total=$(curl -sIL "$URL" | awk 'tolower($0) ~ /^content-length/ {v=$2} END{gsub(/\r/,"",v);print v}')
for i in $(seq 1 80); do
  sz=$(stat -f%z "$OUT" 2>/dev/null || echo 0)
  [ "$sz" -ge "$total" ] && { echo "complete: $sz"; break; }
  curl -sS -L -C - --connect-timeout 15 --max-time 120 "$URL" -o "$OUT" || true
done
```

`-C -` resumes from the partial file; the short `--max-time` per attempt reconnects through each reset. This is the pattern the app's M2 `model_mgr` downloader should implement.
