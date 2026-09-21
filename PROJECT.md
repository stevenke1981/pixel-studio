# Project: Pixel Paint & Viewer 像素畫家與圖片檢視器

## Architecture
Pixel Studio is a Rust workspace consisting of three primary crates:
1. `crates/studio-core`: Headless, zero-UI image processing, filter engine, format conversion, C2PA, layer composition, and the newly added headless raster drawing engine (`drawing.rs`) and canvas border expansion/contraction (`resize.rs`).
2. `crates/studio-app`: Desktop GUI application based on `eframe 0.34.2` / `egui 0.34.2`. Encapsulates `PixelStudioApp`, mode switching (`AppMode`: Viewer, Paint, Batch), top menu chrome, paint tool ribbon, canvas drawing event loop with interactive resize handles, 6-tab inspector panel, and real-time status bar.
3. `crates/studio-cli`: Headless CLI application mirroring core features for smoke testing and headless automation.

### Data Flow & Interaction Pipeline
- **Canvas State**: `working_image: Option<DynamicImage>` (`RgbaImage`) remains the primary raster surface.
- **Drawing Pipeline**: Mouse pointer events on the canvas view translate screen coordinates to image pixel coordinates via `screen_to_image()`. Interactive strokes execute drawing algorithms from `studio_core::drawing` on `working_image`. On mouse release, state is recorded in `image_undo_stack` for full Ctrl+Z / Ctrl+Y undo/redo.
- **Canvas Resize Pipeline**: 3 draggable square handles (right-center, bottom-center, bottom-right) compute target `(new_width, new_height)` and execute `studio_core::resize::canvas_resize()`, preserving existing content and filling new boundaries with background color.
- **Inspector Tabs**: Right panel organized into 6 tabs: Properties (`⚙️ 屬性`), Layers (`📚 圖層`), History (`⏱️ 歷史`), Filters (`✨ 濾鏡`), Info (`📊 資訊`), C2PA (`🛡️ C2PA`).
- **Status Bar Pipeline**: Central canvas interaction updates cursor coordinates `(X, Y)`, samples current pixel color `DynamicImage::get_pixel()`, and updates the status bar with color preview square, hex/RGB text, canvas dimensions, and zoom slider.

---

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Headless Drawing Primitives | Bresenham line, brush with radius, highlighter, airbrush, calligraphy, eraser, flood fill (BFS), color picker, mosaic, shapes in `studio-core` | M1 | Survey 2 |
| 2 | Canvas Non-scaling Resize | Expand/shrink canvas dimensions without scaling existing pixels, fill new boundary with background color | M1 | Survey 2 |
| 3 | Core Drawing Unit Tests | Headless unit tests covering all raster drawing primitives and canvas resize operations | M1 | Survey 2 |
| 4 | Window Title & Main Entry | Update window title to "Pixel Paint & Viewer 像素畫家與圖片檢視器" in `main.rs` | M2 | Survey 1, 3 |
| 5 | Modular i18n TextKeys | Add 54 symmetrical `TextKey` variants with `zh_tw()` and `en()` mappings in `i18n.rs` | M2 | Survey 1, 3 |
| 6 | Top Chrome & 3-Mode Switcher | Top menu bar (`檔案`, `編輯`, `圖片`, `進階濾鏡`, `🌐 C2PA 憑證`, `語言`, `☀️/🌙`) and mode pill buttons (`🖼️ 圖片檢視模式`, `🎨 小畫家繪圖模式` [預設], `⚡ 批次轉檔`) | M2 | Survey 1, 3 |
| 7 | Paint Toolbar Actions & 14 Tools | Common actions (`開始截圖`, `貼上圖片`, `複製畫布`) and 14 tools (`選取`, `鉛筆`, `畫筆`, `螢光標記筆`, `噴槍`, `書法筆`, `橡皮擦`, `油漆桶填色`, `滴管取色`, `文字`, `馬賽克`, `裁切`, `手掌`, `形狀`) | M3 | Survey 1, 3 |
| 8 | Ribbon Options & 30-Color Palette | Stroke thickness dropdown (1, 2, 4, 8, 16px), Color 1 (foreground) / Color 2 (background) dual preview boxes, 30 preset color palette swatches, and custom color picker | M3 | Survey 1, 3 |
| 9 | Default 1280x720 White Canvas | Automatically initialize 1280x720 white canvas when launching or switching to Paint mode without image | M4 | Survey 1, 3 |
| 10 | Canvas Drag Resize Handles | 3 resize handles (right-center, bottom-center, bottom-right) for interactive canvas resizing | M4 | Survey 1, 3 |
| 11 | Canvas Interactive Painting & Undo/Redo | Real-time interactive brush, eraser, flood fill, color picker, shapes on canvas integrated with `image_undo_stack` | M4 | Survey 1, 2 |
| 12 | Inspector 6 Tabs | Right inspector restructured into 6 tabs (`⚙️ 屬性`, `📚 圖層`, `⏱️ 歷史`, `✨ 濾鏡`, `📊 資訊`, `🛡️ C2PA`) | M5 | Survey 1, 3 |
| 13 | Properties Tab Quick Export | Brush radius slider, export format dropdown (PNG/JPEG/WEBP), and "💾 儲存副本" button | M5 | Survey 1, 3 |
| 14 | Real-time Status Bar Metrics | Mode tag, prompt text, coordinates `座標: X, Y px`, hover color square & `#RRGGBBAA (R:xxx G:xxx B:xxx)`, canvas dimensions `畫布: W × H px`, zoom slider `縮放: 100%` | M5 | Survey 1, 3 |
| 15 | Code Quality & Release Build | `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `scripts/build-release.ps1` producing `dist/studio-app.exe` and `dist/studio-cli.exe` | M6 | Survey 2, User Request |
| 16 | Git Commit & Push Authorization | Git commit and push to `origin/main` upon successful verification | M6 | User Request |

---

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Core Drawing & Canvas Resize Engine | `crates/studio-core/src/drawing.rs`, `crates/studio-core/src/resize.rs`, unit tests | None | DONE |
| M2 | Top Chrome, 3-Mode Switcher & i18n | `crates/studio-app/src/main.rs`, `crates/studio-app/src/i18n.rs`, `app.rs` header | None | DONE |
| M3 | Paint Toolbar, Ribbon & Color Palette | `crates/studio-app/src/paint.rs`, `app.rs` toolbar & ribbon rendering | M2 | DONE |
| M4 | Canvas Drawing, Resize Handles & Undo/Redo | `crates/studio-app/src/app.rs`, canvas handle interaction, undo/redo integration | M1, M3 | DONE |
| M5 | Inspector Tabs & Real-time Status Bar | `crates/studio-app/src/app.rs` inspector tabs, status bar metrics | M2, M4 | DONE |
| M6 | E2E Integration, Verification & Release | Full integration testing, fmt, clippy, test suite, release build script, git commit & push | M1–M5 | DONE |

---

## Code Layout & Ownership
To prevent concurrent worker collisions, each milestone has strictly defined write boundaries:
- **M1 Worker Ownership**:
  - `crates/studio-core/src/drawing.rs` (new file)
  - `crates/studio-core/src/resize.rs` (add `canvas_resize`)
  - `crates/studio-core/src/lib.rs` (export `drawing`)
  - `crates/studio-core/tests/` (add drawing tests if needed)
- **M2 Worker Ownership**:
  - `crates/studio-app/src/main.rs` (window title)
  - `crates/studio-app/src/i18n.rs` (add 54 bilingual keys)
- **M3 Worker Ownership**:
  - `crates/studio-app/src/paint.rs` (new module for paint tools, ribbon state, 30 color palette)
  - `crates/studio-app/src/lib.rs` (if applicable) or `crates/studio-app/src/main.rs` (module registration)
- **M4 Worker Ownership**:
  - `crates/studio-app/src/app.rs` (canvas rendering, handles, mouse interaction loop, undo/redo)
- **M5 Worker Ownership**:
  - `crates/studio-app/src/app.rs` (inspector tabs method, status bar method)
- **M6 Worker Ownership**:
  - Full workspace verification, `scripts/build-release.ps1` verification, git operations.

---

## Interface Contracts

### 1. `studio_core::drawing`
```rust
use image::{Rgba, RgbaImage};

pub fn draw_pencil_line(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba<u8>);
pub fn draw_brush_stroke(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, radius: f32, color: Rgba<u8>);
pub fn draw_highlighter_stroke(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, radius: f32, color: Rgba<u8>, alpha: u8);
pub fn draw_airbrush(img: &mut RgbaImage, x: i32, y: i32, radius: f32, density: u32, color: Rgba<u8>);
pub fn draw_calligraphy_stroke(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, width: f32, angle_deg: f32, color: Rgba<u8>);
pub fn draw_eraser(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, radius: f32, bg_color: Rgba<u8>);
pub fn flood_fill(img: &mut RgbaImage, start_x: u32, start_y: u32, fill_color: Rgba<u8>, tolerance: u8);
pub fn sample_color(img: &RgbaImage, x: u32, y: u32) -> Option<Rgba<u8>>;
pub fn apply_mosaic_region(img: &mut RgbaImage, x: u32, y: u32, width: u32, height: u32, block_size: u32);
```

### 2. `studio_core::resize::canvas_resize`
```rust
use image::{Rgba, RgbaImage};

pub fn canvas_resize(
    img: &RgbaImage,
    new_width: u32,
    new_height: u32,
    bg_color: Rgba<u8>,
) -> RgbaImage;
```

### 3. `studio_app::paint`
```rust
use eframe::egui::Color32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppMode {
    Viewer,
    Paint,
    Batch,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaintTool {
    Select,
    Pencil,
    Brush,
    Highlighter,
    Airbrush,
    Calligraphy,
    Eraser,
    FloodFill,
    ColorPicker,
    Text,
    Mosaic,
    Crop,
    Hand,
    Shapes,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InspectorTab {
    Properties,
    Layers,
    History,
    Filters,
    Info,
    C2pa,
}

pub const PRESET_PALETTE_30: [Color32; 30] = [
    // Row 1 (15 colors)
    Color32::from_rgb(0, 0, 0),       // Black
    Color32::from_rgb(128, 128, 128), // Gray
    Color32::from_rgb(136, 0, 21),    // Dark Red
    Color32::from_rgb(237, 28, 36),   // Red
    Color32::from_rgb(255, 127, 39),  // Orange
    Color32::from_rgb(255, 242, 0),   // Yellow
    Color32::from_rgb(34, 177, 76),   // Green
    Color32::from_rgb(0, 162, 232),   // Cyan / Sky Blue
    Color32::from_rgb(63, 72, 204),   // Blue
    Color32::from_rgb(163, 73, 164),  // Purple
    Color32::from_rgb(185, 122, 87),  // Brown
    Color32::from_rgb(255, 174, 201), // Light Pink
    Color32::from_rgb(255, 201, 14),  // Gold
    Color32::from_rgb(239, 228, 176), // Light Yellow
    Color32::from_rgb(181, 230, 29),  // Lime
    // Row 2 (15 colors)
    Color32::from_rgb(255, 255, 255), // White
    Color32::from_rgb(195, 195, 195), // Light Gray
    Color32::from_rgb(190, 30, 45),   // Brick Red
    Color32::from_rgb(255, 80, 80),   // Coral Red
    Color32::from_rgb(255, 160, 60),  // Light Orange
    Color32::from_rgb(255, 255, 120), // Pale Yellow
    Color32::from_rgb(100, 220, 100), // Pale Green
    Color32::from_rgb(153, 217, 234), // Light Cyan
    Color32::from_rgb(112, 146, 190), // Slate Blue
    Color32::from_rgb(200, 191, 231), // Lavender
    Color32::from_rgb(138, 86, 58),   // Dark Brown
    Color32::from_rgb(255, 105, 180), // Hot Pink
    Color32::from_rgb(218, 165, 32),  // Dark Goldenrod
    Color32::from_rgb(46, 139, 87),   // Sea Green
    Color32::from_rgb(30, 144, 255),  // Dodger Blue
];
```
