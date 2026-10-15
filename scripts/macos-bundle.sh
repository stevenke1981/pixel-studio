#!/usr/bin/env bash
# Build a universal (arm64 + x86_64) "Pixel Studio.app" and zip it into dist/.
set -euo pipefail
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d '"' -f2)
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --locked --target aarch64-apple-darwin
cargo build --release --locked --target x86_64-apple-darwin
./scripts/fetch-models.sh
mkdir -p dist/universal
for b in pixel-studio studio-cli; do
  lipo -create -output dist/universal/$b \
    target/aarch64-apple-darwin/release/$b target/x86_64-apple-darwin/release/$b
done
lipo -info dist/universal/pixel-studio

# icon
ICONSET=dist/PixelStudio.iconset
mkdir -p "$ICONSET"
for s in 16 32 128 256; do
  sips -z $s $s crates/studio-app/assets/icon-256.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  d=$((s*2)); [ $d -le 256 ] && sips -z $d $d crates/studio-app/assets/icon-256.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o dist/PixelStudio.icns

APP="dist/Pixel Studio.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS/models" "$APP/Contents/Resources"
cp dist/universal/pixel-studio dist/universal/studio-cli "$APP/Contents/MacOS/"
cp models/*.onnx "$APP/Contents/MacOS/models/"
cp dist/PixelStudio.icns "$APP/Contents/Resources/"
sed "s/__VERSION__/$VERSION/g" scripts/Info.plist > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
codesign --force --deep --sign - "$APP" || true

cd dist
ditto -c -k --sequesterRsrc --keepParent "Pixel Studio.app" pixel-studio-macos-universal-app.zip
mkdir -p pixel-studio-macos-universal/models
cp ../models/*.onnx pixel-studio-macos-universal/models/
cp universal/pixel-studio universal/studio-cli ../README.md ../README.en.md ../LICENSE pixel-studio-macos-universal/
tar czf pixel-studio-macos-universal.tar.gz pixel-studio-macos-universal
ls -la
