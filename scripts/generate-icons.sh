#!/usr/bin/env bash
# Generate square derivatives from the approved 1254x1254 Lunola icon.
# Existing files and approved originals are never overwritten.
#
# Usage:
#   scripts/generate-icons.sh [source.png] [output-dir]
#
# Defaults:
#   source     assets/lunola/originals/app-icon.png
#              (approved original; never overwrite it)
#   output-dir assets/lunola/generated-icons
#
# Uses sips on macOS, ImageMagick (magick/convert) elsewhere.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="${1:-$REPO_ROOT/assets/lunola/originals/app-icon.png}"
OUT="${2:-$REPO_ROOT/assets/lunola/generated-icons}"
SIZES=(1024 512 256 128 64 32 16)

if [[ ! -f "$SRC" ]]; then
  echo "error: source image not found: $SRC" >&2
  exit 1
fi

# Derivatives must never overwrite approved originals or the input image.
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd -P)"
case "$OUT/" in
  "$REPO_ROOT/assets/lunola/originals/"*)
    echo "error: approved originals are immutable; use a derivative directory" >&2
    exit 1 ;;
esac

resize() { # resize <size> <dest>
  if command -v sips >/dev/null 2>&1; then
    sips -z "$1" "$1" "$SRC" --out "$2" >/dev/null
  elif command -v magick >/dev/null 2>&1; then
    magick "$SRC" -resize "${1}x${1}" "$2"
  elif command -v convert >/dev/null 2>&1; then
    convert "$SRC" -resize "${1}x${1}" "$2"
  else
    echo "error: need sips or ImageMagick (magick/convert) on PATH" >&2
    exit 1
  fi
}

for size in "${SIZES[@]}"; do
  dest="$OUT/icon-${size}.png"
  if [[ -e "$dest" || -L "$dest" ]]; then
    echo "error: refusing to overwrite existing derivative: $dest" >&2
    exit 1
  fi
  resize "$size" "$dest"
  echo "wrote $dest"
done
