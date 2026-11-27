# Pixel Studio — Agent Notes (`AGENTS.md`)

Rust + Slint cross-platform image editor.
Designed to be **Human-First** (bilingual `zh-TW` / `en` GUI, intuitive Paint-surpassing tools, non-destructive layers, privacy-safe defaults) and **Agent-Friendly** (UI-independent core & document model, full CLI parity, deterministic `--json` output, strict quality gates).

## Workspace Layout

- `crates/studio-core/` — Pure Rust image processing library (zero UI dependencies):
  - `atomic_file.rs` — staging-file atomic write helpers (`write`, `write_with` with staging callback) so failed exports never corrupt existing files; GIF and panorama outputs use them too.
  - `batch.rs` — parallel batch conversion, resize, watermark, and C2PA batch sign/remove. C2PA batch accepts arbitrary multi-file lists with dedup and collision-free output names (`_c2pa`, `_c2pa_2`…), never overwriting.
  - `c2pa_metadata.rs` — C2PA content credentials inspection (`inspect_c2pa`), signing (`write_c2pa_manifest`), and removal (`remove_c2pa_manifest`).
  - `compose.rs` — non-destructive layer composition (`TextLayer`, `ImageLayer`, `ShapeLayer` with `Line`, `Arrow`, `Rectangle`, `Ellipse`, `Triangle`), 10 `BlendMode` variants (`Normal`, `Multiply`, `Screen`, `Overlay`, `Darken`, `Lighten`, `SoftLight`, `ColorDodge`, `Difference`, `Exclusion`), and layer transforms (`flip_layer_horizontal`, `flip_layer_vertical`, `rotate_layer_90`).
  - `convert.rs` & `formats.rs` — format conversion (`png`, `jpg`, `jfif`, `bmp`, `gif`, `ico`, `webp` lossless/lossy, `tiff`).
  - `drawing.rs` — raster canvas tools (pencil, brush, highlighter, airbrush, calligraphy, eraser, flood fill, color picker, rectangle/circle/ellipse, mosaic).
  - `filters.rs` — `Filter` trait, `FilterPipeline`, and filter structs/functions (`Rotate180`, `HueAdjust`/`adjust_hue`, `Sepia`/`sepia`, `Vignette`/`vignette`, `TemperatureAdjust`/`adjust_temperature`, `TintAdjust`/`adjust_tint`, `AutoContrast`/`auto_contrast`, `Posterize`/`posterize`). Per-pixel filters are rayon-parallel.
  - `gif.rs` — animated GIF encoder from image sequences.
  - `image_io.rs` — image file inspection (`inspect_image` → `ImageInfo`: format, dimensions, color type, file size).
  - `gpano_xmp.rs` & `panorama360.rs` — 2:1 equirectangular 360° panorama builder (`pad`, `crop`, `stretch`) and GPano XMP injector.
  - `metadata.rs` — EXIF/ICC/XMP metadata inspection and preservation (exports strip metadata by default unless opted in).
  - `naming.rs` — local timestamp filename generator (`pixelstudio_YYYYMMDD_HHMMSS.<ext>`) and `create_unique_output_dir` (creates a `<prefix>_YYYYMMDD_HHMMSS` folder, never reuses an existing one).
  - `ocr.rs` & `redact.rs` — offline text detection (`ocrs` + `rten`) and sensitive data redaction (TW/intl phone, email, TW National ID with checksum, Luhn credit card, URL, address; solid/mosaic/blur styles).
  - `pro_adjust.rs` — histogram analysis & tonal grading: `HistogramData` (`compute_histogram`, `render_histogram_image`), `LevelsAdjust`/`adjust_levels`, `ShadowsHighlights`/`adjust_shadows_highlights`, `VibranceAdjust`/`adjust_vibrance`, `ClarityAdjust`/`adjust_clarity`, `ColorBalance`/`adjust_color_balance`, `Threshold`/`threshold`, and 8 `ColorLook` presets via `LookFilter`/`apply_color_look` (`Vivid`, `CinematicTealOrange`, `MatteFilm`, `MoodyBw`, `WarmGolden`, `CrispCool`, `HighKey`, `Cyberpunk`).
  - `pro_effects.rs` — smart cutout, framing, spatial/stylize filters, and raster tools: `magic_wand_mask`, `remove_background_color`, `auto_trim` (`TrimMode`, `TrimResult`), `round_corners`, `add_drop_shadow`, `add_border_frame` (`FrameCardOptions`), `Filter` structs/functions (`TiltShift`/`tilt_shift`, `Pixelate`/`pixelate`, `FilmGrain`/`film_grain`, `Emboss`/`emboss`, `EdgeDetect`/`edge_detect`, `OilPaint`/`oil_paint`), `draw_gradient` (`GradientKind::Linear`/`Radial`), and `clone_stamp_dab`.
  - `resize.rs` — `resize_exact`, `resize_to_fit`, and `canvas_resize`.
  - `stickers.rs` — 65 original MIT-licensed SVG stickers rendered via `resvg`/`tiny-skia` with recolor, outline, and drop shadow.
  - `transform.rs` — `Transform` trait, `TransformPipeline`, 12 transform structs (`RotateLeft`, `RotateRight`, `FlipHorizontal`, `FlipVertical`, `BrightnessAdjust`, `ContrastAdjust`, `SaturationAdjust`, `ExposureAdjust`, `Blur`, `Sharpen`, `Grayscale`, `Invert`), plus `crop_region`, `blur_region`, and `SelectionMask`.
  - `util.rs` — shared channel math (`clamp_channel`, `lerp_channel`, `rgb_to_hsl`, `hsl_to_rgb`).
- `crates/studio-app/` — Slint desktop GUI application (binary name: `pixel-studio`):
  - `src/doc.rs` — UI-independent document state, 20 tools (including `Crop` with Rect/Circle/Freehand modes, `Tape` sticker brush, `Gradient`, `MagicWand`, and `Clone`), bitmap `selection_mask`, layers with `BlendMode` & productivity ops (`set_selected_blend_mode`, `center_selected_layer`, `flip_selected_layer`, `rotate_selected_layer`, `merge_selected_down`), pro ops (`compute_histogram`, `apply_remove_background`, `apply_auto_trim`, `apply_frame_card`), undo/redo history, redaction box selection/deletion, and `.pixel-project` v2 serialization.
  - `src/ui.rs` & `ui/app.slint` — Slint UI adapter, 132 original SVG icons (`ui/icons/`; 131 generated via `scripts/gen-icons.py` + `tool-tape.svg`), collapsible inspector (46px icon rail, F4, persisted), focus mode (Tab hides tool column + inspector, transient), multi-image C2PA add/strip into a new folder from the C2PA dialog, Windows CJK font fallback (`src/fonts.rs`), and bilingual strings (`zh-TW` default on Chinese locales, `en` elsewhere, override via `PIXEL_STUDIO_LANG=en|zh`).
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

On Windows the Skia backend needs the MSVC toolchain. The default toolchain here is `stable-x86_64-pc-windows-gnu`, so prefix commands with `cargo +stable-x86_64-pc-windows-msvc` if they fail.

## Agent-Friendly CLI Reference (`studio-cli`)

Pass `--json` to any subcommand for structured JSON output on stdout:

```powershell
# Inspect image metadata & dimensions
cargo run -p studio-cli -- --json info input.png

# Compute RGB & BT.709 luminance histogram (add --bins for 256-bin arrays)
cargo run -p studio-cli -- --json histogram input.png --bins

# Apply composable transforms & filters in a single pass
cargo run -p studio-cli -- --json filter input.png output.png --rotate 90 --brightness 10 --contrast 15 --sepia 0.4 --vignette 0.2 --auto-contrast

# Pro tonal grading & color looks via filter
cargo run -p studio-cli -- --json filter input.png output.png --levels "10,1.1,245" --shadows 20 --highlights -15 --vibrance 25 --clarity 30 --color-balance "15,-5,-10" --look cinematic
cargo run -p studio-cli -- --json filter input.png output.png --threshold 128

# Pro spatial & stylize effects via filter
cargo run -p studio-cli -- --json filter input.png output.png --tilt-shift "0.5,0.25,6.0" --grain 0.25 --pixelate 8 --emboss 0.8 --oil-paint 3
cargo run -p studio-cli -- --json filter input.png output.png --edge-detect
cargo run -p studio-cli -- --json filter input.png output.png --sketch

# Remove solid background color, auto-trim margins, or frame on a shadow card
cargo run -p studio-cli -- --json remove-bg input.jpg cutout.png --color "#FFFFFF" --tolerance 32 --feather 8 --edge-only
cargo run -p studio-cli -- --json trim input.png trimmed.png --corner-tolerance 12
cargo run -p studio-cli -- --json frame input.png card.png --padding 24 --round-corners 16 --shadow-sigma 12 --shadow-x 0 --shadow-y 6 --background "#F4F6F9FF" --border-width 1 --border-color "#D0D7DEFF"

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

# Multi-file C2PA: inputs may be files or directories; `--new-folder` creates `pixelstudio_c2pa_added_*`/`pixelstudio_c2pa_removed_*` under `--output-dir` (default: first input's folder)
cargo run -p studio-cli -- --json c2pa-add-files a/photo.jpg b/photo.jpg --new-folder --output-dir ./out --title "Verified" --creator "Pixel Studio"
cargo run -p studio-cli -- --json c2pa-remove-files a.jpg b.jpg --output-dir ./clean

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
- **Atomic file writes**: Use `studio_core::atomic_file::write` (or `write_with` for a staging callback) when saving outputs so partial writes never clobber existing files.
