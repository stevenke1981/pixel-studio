#!/usr/bin/env bash
# Capture GUI screenshots under Xvfb: main window, stickers tab, redaction overlay.
# Usage: scripts/linux-screenshots.sh <pixel-studio binary> <out dir>
set -euo pipefail
BIN=${1:-target/release/pixel-studio}
OUT=${2:-docs}
mkdir -p "$OUT"
SAMPLE=${SAMPLE:-crates/studio-core/tests/fixtures/redact_sample.png}
export DISPLAY=${DISPLAY_NUM:-:97}
Xvfb "$DISPLAY" -screen 0 1440x900x24 >/dev/null 2>&1 &
XVFB=$!
trap 'kill $XVFB 2>/dev/null || true' EXIT
sleep 1.5
shot() { # name tab wait [extra env...]
  local name=$1 tab=$2 wait=$3; shift 3
  env PIXEL_STUDIO_LANG=zh-TW PIXEL_STUDIO_TAB="$tab" "$@" "$BIN" "$SAMPLE" >/tmp/ps-$name.log 2>&1 &
  local pid=$!
  sleep "$wait"
  if ! kill -0 $pid 2>/dev/null; then echo "app exited early:"; cat /tmp/ps-$name.log; exit 1; fi
  import -window root "$OUT/$name.png"
  convert "$OUT/$name.png" -crop 1400x860+0+0 +repage "$OUT/$name.png" || true
  kill $pid || true
  wait $pid 2>/dev/null || true
  echo "captured $OUT/$name.png"
}
shot slint-main 1 4
shot slint-stickers 3 4
shot slint-redact 4 ${REDACT_WAIT:-25} PIXEL_STUDIO_AUTODETECT=1
