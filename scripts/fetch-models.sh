#!/usr/bin/env bash
# Download the offline OCR models (ocrs, ~12 MB) into models/ for bundling.
set -euo pipefail
mkdir -p models
for m in text-detection text-recognition; do
  [ -f "models/$m.onnx" ] || curl -sSfL --retry 3 -o "models/$m.onnx" "https://ocrs-models.s3-accelerate.amazonaws.com/$m.onnx"
done
ls -la models
