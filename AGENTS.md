# Pixel Studio — Agent Notes (`AGENTS.md`)

Rust + Slint cross-platform image editor (`D:\pixel-studio`).
Designed to be **Human-First** (bilingual `zh-TW` / `en` GUI, intuitive Paint-surpassing tools, non-destructive layers, privacy-safe defaults) and **Agent-Friendly** (UI-independent core & document model, full CLI parity, deterministic `--json` output, strict quality gates).

## Workspace Layout

- `crates/studio-core/` — Pure Rust image processing library (zero UI dependencies):
  - `atomic_file.rs` — staging-file atomic write helper so failed exports never corrupt existing files.
  - `batch.rs` — parallel batch conversion, resize, watermark, and C2PA batch sign/remove.
  - `c2pa_metadata.rs` — C2PA content credentials inspection (`inspect_c2pa`), signing (`write_c2pa_manifest`), and removal (`remove_c2pa_manifest`).
  - `compose.rs` — non-destructive layer composition (`TextLayer`, `ImageLayer`, `ShapeLayer` with `Line`, `Arrow`, `Rectangle`, `Ellipse`, `Triangle`).
  - `convert.rs` & `formats.rs` — format conversion (`png`, `jpg`, `jfif`, `bmp`, `gif`, `ico`, `webp` lossless/lossy, `tiff`).
  - `drawing.rs` — raster canvas tools (pencil, brush, highlighter, airbrush, calligraphy, eraser, flood fill, color picker, rectangle/circle/ellipse, mosaic).
  - `filters.rs` — `Filter` trait, `FilterPipeline`, and filter structs/functions (`Rotate180`, `HueAdjust`/`adjust_hue`, `Sepia`/`sepia`, `Vignette`/`vignette`, `TemperatureAdjust`/`adjust_temperature`, `TintAdjust`/`adjust_tint`, `AutoContrast`/`auto_contrast`, `Posterize`/`posterize`).
  - `gif.rs` — animated GIF encoder from image sequences.
  - `gpano_xmp.rs` & `panorama360.rs` — 2:1 equirectangular 360° panorama builder (`pad`, `crop`, `stretch`) and GPano XMP injector.
  - `metadata.rs` — EXIF/ICC/XMP metadata inspection and preservation (exports strip metadata by default unless opted in).
  - `naming.rs` — local timestamp filename generator (`pixelstudio_YYYYMMDD_HHMMSS.<ext>`).
  - `ocr.rs` & `redact.rs` — offline text detection (`ocrs` + `rten`) and sensitive data redaction (TW/intl phone, email, TW National ID with checksum, Luhn credit card, URL, address; solid/mosaic/blur styles).
  - `resize.rs` — `resize_exact`, `resize_to_fit`, and `canvas_resize`.
  - `stickers.rs` — 65 original MIT-licensed SVG stickers rendered via `resvg`/`tiny-skia` with recolor, outline, and drop shadow.
  - `transform.rs` — `Transform` trait, `TransformPipeline`, 12 transform structs (`RotateLeft`, `RotateRight`, `FlipHorizontal`, `FlipVertical`, `BrightnessAdjust`, `ContrastAdjust`, `SaturationAdjust`, `ExposureAdjust`, `Blur`, `Sharpen`, `Grayscale`, `Invert`), plus `crop_region`, `blur_region`, and `SelectionMask`.
  - `util.rs` — shared channel math (`clamp_channel`, `lerp_channel`, `rgb_to_hsl`, `hsl_to_rgb`).
- `crates/studio-app/` — Slint desktop GUI application (binary name: `pixel-studio`):
  - `src/doc.rs` — UI-independent document state, tools (17 tools including `Crop` with Rect/Circle/Freehand modes and `Tape` sticker brush), layers, undo/redo history, redaction box selection/deletion, and `.pixel-project` serialization.
  - `src/ui.rs` & `ui/app.slint` — Slint UI adapter, 115 original SVG icons (`ui/icons/`), Windows CJK font fallback (`src/fonts.rs`), and bilingual strings (`zh-TW` default on Chinese locales, `en` elsewhere, override via `PIXEL_STUDIO_LANG=en|zh`).
  - `src/tape.rs` — 10 procedural pattern tape sticker brushes.
  - `src/capture.rs` — cross-platform screen capture.
  - `src/shell.rs` — per-user OS context menu and "Open With" registration (`--register-shell` / `--unregister-shell`).
- `crates/studio-cli/` — Headless CLI (`studio-cli`) exposing all `studio-core` operations with human-readable output by default and `--json` for AI agents and scripts.

`Cargo.toml` at repo root is a virtual workspace; never add a `[package]` section there.

## Build, Verify & Release Commands

Run all three verification gates before committing:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Build optimized release binaries (`target/release/pixel-studio.exe` and `target/release/studio-cli.exe`):

```powershell
cargo build --release --workspace
```

## Agent-Friendly CLI Reference (`studio-cli`)

Pass `--json` to any subcommand for structured JSON output on stdout:

```powershell
# Inspect image metadata & dimensions
cargo run -p studio-cli -- --json info input.png

# Apply composable transforms & filters in a single pass
cargo run -p studio-cli -- --json filter input.png output.png --rotate 90 --brightness 10 --contrast 15 --sepia 0.4 --vignette 0.2 --auto-contrast

# Resize image (exact, --fit, or --canvas with hex fill)
cargo run -p studio-cli -- --json resize input.png output.png --width 1080 --height 1080 --fit
cargo run -p studio-cli -- --json resize input.png output.png --width 1200 --height 800 --canvas --background "#FFFFFFFF"

# Convert format (png, jpg, jfif, bmp, gif, ico, webp, tiff)
cargo run -p studio-cli -- --json convert input.png output.webp --format webp --quality 90

# Crop or blur a rectangular region
cargo run -p studio-cli -- --json crop input.png output.png --x 10 --y 10 --width 200 --height 150
cargo run -p studio-cli -- --json blur-region input.png output.png --x 10 --y 10 --width 200 --height 60 --sigma 12

# Offline OCR auto-redaction
cargo run -p studio-cli -- --json redact input.png output.png --style solid --keyword secret

# 360° equirectangular JPEG with GPano XMP
cargo run -p studio-cli -- --json pano360 input.jpg output_360.jpg --mode pad

# C2PA content credentials (inspect, single-file remove, batch add/remove)
cargo run -p studio-cli -- --json c2pa-info input.jpg
cargo run -p studio-cli -- --json c2pa-remove-file input.jpg clean.jpg
cargo run -p studio-cli -- --json c2pa-add ./in_dir ./out_dir --title "Verified" --creator "Pixel Studio"
cargo run -p studio-cli -- --json c2pa-remove ./in_dir ./out_dir

# Batch convert & animated GIF
cargo run -p studio-cli -- --json batch ./in_dir ./out_dir --format webp --max-width 1920
cargo run -p studio-cli -- --json gif ./frames_dir anim.gif --delay-ms 100
```

## GUI Binary CLI Flags (`pixel-studio`)

```powershell
pixel-studio [--edit|--view] [FILE]
pixel-studio --register-shell
pixel-studio --unregister-shell
pixel-studio --version
pixel-studio --help
```

## Conventions & Gotchas

- **Keep `studio-core` UI-free**: Never add Slint or windowing dependencies to `studio-core`.
- **Keep `doc.rs` testable**: New editor tools or layer behaviors belong in `crates/studio-app/src/doc.rs` with unit tests before wiring into `ui.rs` / `ui/app.slint`.
- **Privacy by default**: Exports strip EXIF/XMP metadata unless `keep_metadata` is explicitly enabled; solid-box redaction is the default over mosaic/blur.
- **Atomic file writes**: Use `studio_core::atomic_file::write_atomic` when saving outputs so partial writes never clobber existing files.
