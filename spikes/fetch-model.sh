#!/usr/bin/env bash
# Fetch a ggml Whisper model into ./models for the S1 spike.
#
# Usage:  ./fetch-model.sh [base|small|medium]   (default: small)
#
# Models are pulled from the official whisper.cpp Hugging Face repo and are the
# same files the real app (M2 model_mgr.rs) will download on demand.
set -euo pipefail

MODEL="${1:-small}"
case "$MODEL" in
  base|small|medium) ;;
  *) echo "unknown model '$MODEL' (use: base | small | medium)" >&2; exit 1 ;;
esac

DIR="$(cd "$(dirname "$0")" && pwd)/models"
mkdir -p "$DIR"
OUT="$DIR/ggml-${MODEL}.bin"
URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-${MODEL}.bin"

if [ -f "$OUT" ]; then
  echo "already present: $OUT"
  exit 0
fi

echo "downloading ggml-${MODEL}.bin -> $OUT"
curl -L --fail --progress-bar "$URL" -o "$OUT"
echo "done: $OUT"
