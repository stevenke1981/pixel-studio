# TODOs — Pixel Studio UI Design Redesign for Codex

> Target repo: `https://github.com/stevenke1981/pixel-studio.git`
> Goal: modify the existing Rust + egui desktop app so the UI visually matches the provided `UI_DESIGN_SPEC.md` and `ui_prototype.html` reference design.
> Main crate to edit: `crates/studio-app/`
> Preserve existing image editing / conversion / C2PA / project-file behavior unless a UI wiring change is explicitly required.

## Implementation status — 2026-06-21

- [x] Added the modular Rust + egui UI under `crates/studio-app/src/ui/`.
- [x] Replaced the root editor shell with 40px top menu, 34px contextual toolbar, 50px tool rail, 260px right panel, center canvas, and 26px status bar.
- [x] Kept brush and line drawing visible in the first viewport and synchronized with the existing raster drawing engine.
- [x] Wired real layers, adjustment commits, undo/redo, export, zoom, bilingual labels, and keyboard tool switching.
- [x] Kept all specialist workflows reachable through `檢視 > 完整工具面板`.
- [x] Verified formatting, strict clippy, 33 workspace tests, GUI startup, and a real egui frame screenshot at 1440×960 logical size.

Follow-up work: implement raster behavior for Eraser/Stamp/Eyedropper/Shape, named history entries, live non-destructive adjustment previews, and a second manual screenshot pass under a dark Windows theme.

### Left tool migration — 2026-06-21

- [x] Migrated every legacy workflow into nine left categories: File, Browse, Drawing, Edit, Layers, Export, GIF, Batch, and C2PA.
- [x] Replaced the floating legacy tools window with a persistent 50px category rail and optional 300px drawer.
- [x] Kept Brush and Line visible in the default Drawing category with named tool buttons and shortcuts.
- [x] Removed duplicated C2PA controls from Browse and kept C2PA single/batch workflows in one category.
- [x] Built and smoke-tested the Windows Release binaries in `dist/`.

### Common toolbar — 2026-07-08

- [x] Generated a Codex built-in GUI reference image at `docs/assets/pixel-studio-toolbar-reference.png`.
- [x] Added top-toolbar shortcuts for open, paste, save, save as, undo, redo, crop, brush, line, fit, 100%, and export.
- [x] Styled active tool shortcuts with the existing blue selected state while keeping menus and left categories available.
- [x] Re-run formatting, clippy, tests, release build, and GUI smoke test after the toolbar change.

---

## 0. Read first

Codex must inspect these files before editing:

1. `AGENTS.md`
2. `README.md`
3. `Cargo.toml`
4. `crates/studio-app/src/main.rs`
5. `crates/studio-app/src/app.rs`
6. `crates/studio-app/src/i18n.rs`
7. `crates/studio-core/src/compose.rs`
8. `crates/studio-core/src/filters.rs`
9. Reference files supplied by user:
   - `UI_DESIGN_SPEC.md`
   - `ui_prototype.html`

Important repo facts:

- This is a Rust workspace with `crates/studio-core`, `crates/studio-cli`, and `crates/studio-app`.
- `studio-app` uses `eframe` / `egui`; do **not** rewrite the app in React even though the UI spec mentions React as a reference stack.
- `app.rs` is large; refactor UI code into smaller modules instead of adding more large functions into `app.rs`.
- Default UI language is Traditional Chinese. Any new user-facing label must be added to `TextKey` and implemented in both `Language::ZhTw` and `Language::En` arms in `crates/studio-app/src/i18n.rs`.
- Root `Cargo.toml` is workspace-only; do not add `[package]` there.

---

## 1. Design target

Implement a desktop image editor layout inspired by the provided prototype:

```text
Pixel Studio window
├─ TopMenuBar              height 40px
├─ ContextualToolbar       height 34px
├─ Workspace               flex / fills remaining space
│  ├─ ToolRail             width 50px
│  ├─ Canvas               fills center area
│  └─ RightPanel           width 260px
│     ├─ PanelTabs         Layers / Adjust / History
│     └─ active panel only
└─ StatusBar               height 26px
```

Visual style:

- Flat UI.
- No gradient.
- No heavy shadow.
- Thin borders, approximately `0.5px` where egui allows it; use `1.0px` only where egui rendering makes `0.5px` too faint.
- Tight spacing.
- Rounded corners: medium `8px`, large `12px`.
- Typography: 11–13px labels/body, only normal and medium emphasis.
- Support both light and dark mode.

---

## 2. Add UI module structure

Create a dedicated UI module under `crates/studio-app/src/ui/`:

```text
crates/studio-app/src/ui/
├─ mod.rs
├─ theme.rs
├─ state.rs
├─ top_menu_bar.rs
├─ contextual_toolbar.rs
├─ tool_rail.rs
├─ canvas_view.rs
├─ right_panel.rs
└─ status_bar.rs
```

Update `crates/studio-app/src/main.rs` or `app.rs` module declarations as needed:

```rust
mod ui;
```

Keep business logic in existing app/core code. The new `ui` module should mainly contain:

- layout drawing
- theme tokens
- UI state structs/enums
- reusable egui widgets
- small adapter functions to call existing app behavior

---

## 3. Implement theme tokens in `ui/theme.rs`

Create a semantic token layer that maps the prototype CSS variables to egui values.

Required tokens:

```rust
pub struct UiTokens {
    pub bg_primary: egui::Color32,
    pub bg_secondary: egui::Color32,
    pub bg_tertiary: egui::Color32,
    pub bg_info: egui::Color32,
    pub text_primary: egui::Color32,
    pub text_secondary: egui::Color32,
    pub text_tertiary: egui::Color32,
    pub text_info: egui::Color32,
    pub border_tertiary: egui::Color32,
    pub border_secondary: egui::Color32,
    pub border_info: egui::Color32,
    pub radius_md: u8,
    pub radius_lg: u8,
}
```

Light values:

```text
bg_primary      #ffffff
bg_secondary    #f5f5f3
bg_tertiary     #ececea
bg_info         #E6F1FB
text_primary    #1a1a1a
text_secondary  #6b6b66
text_tertiary   #9a9893
text_info       #0C447C
border_tertiary rgba(0,0,0,.08)
border_secondary rgba(0,0,0,.15)
border_info     #378ADD
radius_md       8
radius_lg       12
```

Dark values:

```text
bg_primary      #1c1c1a
bg_secondary    #262624
bg_tertiary     #2f2f2c
bg_info         #042C53
text_primary    #f1efe8
text_secondary  #b4b2a9
text_tertiary   #888780
text_info       #85B7EB
border_tertiary rgba(255,255,255,.10)
border_secondary rgba(255,255,255,.18)
border_info     #378ADD
```

Implementation requirements:

- Add `UiTokens::from_ctx(ctx: &egui::Context) -> Self` or equivalent using `ctx.style().visuals.dark_mode`.
- Add `apply_visuals(ctx: &egui::Context)` to set egui visuals globally.
- Use semantic tokens everywhere instead of scattered hard-coded colors.
- Avoid `Frame::default().shadow(...)` unless existing egui defaults need to be neutralized.

---

## 4. Add UI state types in `ui/state.rs`

Create state types that represent the new UI without breaking existing app state.

Required enums:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolId {
    Move,
    Lasso,
    Crop,
    Brush,
    Eraser,
    Stamp,
    Text,
    Shape,
    Eyedropper,
    Zoom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightPanelTab {
    Layers,
    Adjust,
    History,
}
```

Required settings structs:

```rust
#[derive(Debug, Clone)]
pub struct BrushUiSettings {
    pub size_px: u32,       // 1..=200, default 60
    pub hardness: u32,      // 0..=100, default 80
    pub opacity: u32,       // 0..=100, default 100
    pub blend_mode: BlendModeUi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendModeUi {
    Normal,
    Multiply,
    Screen,
}

#[derive(Debug, Clone)]
pub struct AdjustUiValues {
    pub brightness: i32,    // 0..=100, default 50
    pub contrast: i32,      // 0..=100, default 50
    pub saturation: i32,    // 0..=100, default 50
    pub temperature: i32,   // 0..=100, default 50
}
```

Add these fields to `PixelStudioApp` or a nested `UiState`:

```rust
active_tool: ToolId,
right_panel_tab: RightPanelTab,
brush_ui: BrushUiSettings,
adjust_ui: AdjustUiValues,
zoom_percent: u32,
```

Do not remove existing undo/redo/history structures. If existing fields already overlap, adapt instead of duplicating long-term state.

---

## 5. Implement `TopMenuBar`

File: `ui/top_menu_bar.rs`

Target:

- height: `40.0`
- left: app icon-like label + `Pixel Studio`
- menu items: `檔案`, `編輯`, `影像`, `圖層`, `濾鏡`, `檢視`
- right: undo, redo, export button

Implementation notes:

- egui does not have Tabler Icons built in. Use simple text glyphs for now, for example:
  - app: `▣` or `◰`
  - undo: `↶`
  - redo: `↷`
- Keep icon-only buttons accessible by hover text.
- Wire undo/redo/export to existing app methods if present. If names are unclear, add TODO comments near the adapter call and keep the button visible but disabled only when no action is possible.
- Use `tokens.bg_primary`, `tokens.border_tertiary`, and `tokens.text_secondary`.

Acceptance:

- Top bar visually replaces any older cluttered top controls.
- Existing file/open/export functionality remains reachable either through this bar or an existing panel until fully migrated.

---

## 6. Implement `ContextualToolbar`

File: `ui/contextual_toolbar.rs`

Target:

- height: `34.0`
- content changes based on `active_tool`
- for Brush/Eraser tools show:
  - `大小` slider `1..=200`, default `60px`
  - `硬度` slider `0..=100`, default `80%`
  - `不透明度` slider `0..=100`, default `100%`
  - blend mode combo: `正常`, `色彩增值`, `濾色`
- for Crop show crop action controls if existing crop code supports it.
- for Move / Text / Shape / Zoom show minimal helpful settings instead of leaving it empty.

Implementation notes:

- Values displayed beside sliders must be rounded integers.
- Map visible labels through `TextKey` where practical.
- This is UI state first; deeper brush engine behavior can be a follow-up if current app does not yet support brush painting.

Acceptance:

- Switching tools changes toolbar content.
- Slider value labels update immediately.

---

## 7. Implement `ToolRail`

File: `ui/tool_rail.rs`

Target:

- width: `50.0`
- vertical list of tool buttons, each 34×34 with 8px radius
- active tool uses `bg_info` + `text_info`
- hover uses `bg_secondary`
- separator after `Stamp`
- `Zoom` button pinned near bottom if layout space allows

Tool list:

| ToolId | zh-TW label | shortcut |
|---|---|---|
| Move | 移動 | V |
| Lasso | 套索 | L |
| Crop | 裁切 | C |
| Brush | 筆刷 | B |
| Eraser | 橡皮擦 | E |
| Stamp | 仿製印章 | S |
| Text | 文字 | T |
| Shape | 圖形 | U |
| Eyedropper | 滴管 | I |
| Zoom | 縮放 | Z |

Implementation notes:

- Use Unicode/text symbols until a real icon system is added:
  - Move `↖`
  - Lasso `〰`
  - Crop `⌗`
  - Brush `✎`
  - Eraser `▱`
  - Stamp `▣`
  - Text `T`
  - Shape `□`
  - Eyedropper `◉`
  - Zoom `⌕`
- Add keyboard shortcuts with egui input handling.
- Do not implement destructive image operations just because the tool is selected. Tool selection should only update active UI mode.

Acceptance:

- Active tool changes on click and shortcut.
- Active styling matches prototype intent.

---

## 8. Implement `CanvasView`

File: `ui/canvas_view.rs`

Target:

- center area fills remaining space.
- background uses `bg_tertiary`.
- current image is centered and scaled by `zoom_percent`.
- 100% means one image pixel maps to one screen pixel when possible.
- bottom-right zoom badge displays e.g. `100%`.
- selection rectangle uses dashed `border_info` stroke.

Implementation notes:

- Reuse existing image texture loading and preview logic from `app.rs`.
- If egui has no dashed rectangle helper, implement manual dashed lines with `Painter::line_segment`.
- Preserve existing pan/zoom behavior if already implemented.
- Keep image display isolated from side panels. Use `min-width: 0` equivalent by avoiding layout that forces the canvas wider than available space.

Acceptance:

- Canvas is visually clean and centered.
- Zoom badge updates when StatusBar zoom slider changes.
- Existing open image preview still works.

---

## 9. Implement `RightPanel`

File: `ui/right_panel.rs`

Target:

- width: `260.0`
- top tabs: `圖層`, `調整`, `紀錄`
- only active panel is drawn.
- active tab has 2px bottom border using `border_info` and `text_primary`.

### 9.1 Layers panel

Rows should contain:

- visible toggle icon/text
- 26×26 thumbnail square
- layer name
- lock indicator for locked/background layer

Also include selected layer opacity slider `0..=100%`.

Use existing layer data if available. If existing layer model differs, create adapter functions and avoid changing core compose format unless required.

### 9.2 Adjust panel

Sliders:

- 亮度 `0..=100`, default 50
- 對比 `0..=100`, default 50
- 飽和度 `0..=100`, default 50
- 色溫 `0..=100`, default 50

Mapping to existing filters:

- `50` means neutral / no change.
- Below 50 maps negative adjustment.
- Above 50 maps positive adjustment.
- Do not permanently mutate the image on every drag unless existing app already uses preview-safe editing. Prefer preview state while dragging, apply through existing command/history path on commit.

### 9.3 History panel

- Show existing edit history entries if available.
- Current/latest state uses `text_info` and medium font weight.
- If existing history is not exposed, show a small adapter TODO and preserve existing undo/redo functionality.

Acceptance:

- Tabs switch reliably.
- Sliders update values immediately.
- Layer list does not overflow the 260px panel.

---

## 10. Implement `StatusBar`

File: `ui/status_bar.rs`

Target:

- height: `26.0`
- left: `{width} × {height} px・{color mode}・{bit depth}`
- right: zoom out icon, zoom slider `10..=400` step `10`, zoom in icon, value text

Implementation notes:

- Use actual image dimensions if an image is loaded.
- Use placeholder `— × — px・RGB・8bit` when no image is loaded.
- Wire zoom slider to the same zoom state used by `CanvasView`.

Acceptance:

- Status bar remains visible at bottom.
- Zoom slider updates canvas and badge.

---

## 11. Recompose main app layout

Modify the `eframe::App` update method in `app.rs` to render the new structure.

Suggested root order:

1. Apply theme once per frame.
2. Draw `TopMenuBar` with `egui::TopBottomPanel::top("top_menu_bar")` and exact height 40.
3. Draw `ContextualToolbar` with another `TopBottomPanel::top("contextual_toolbar")` and exact height 34.
4. Draw `StatusBar` with `TopBottomPanel::bottom("status_bar")` and exact height 26.
5. Draw `ToolRail` with `SidePanel::left("tool_rail")` exact width 50.
6. Draw `RightPanel` with `SidePanel::right("right_panel")` exact width 260.
7. Draw `CanvasView` in the remaining `CentralPanel`.

Use frames with:

- `fill(tokens.bg_primary)` for panels
- `fill(tokens.bg_secondary)` for tool rail/status when appropriate
- thin strokes using `tokens.border_tertiary`
- no heavy shadows

Do not remove existing functional panels until each feature is accessible in the new layout. If needed, temporarily place legacy panels behind a `View > Legacy panels` toggle during migration.

---

## 12. i18n tasks

Update `crates/studio-app/src/i18n.rs`.

Add or reuse text keys for:

```text
File
Edit
Image
Layer
Filter
View
Export
Undo
Redo
Move
Lasso
Crop
Brush
Eraser
Stamp
Text
Shape
Eyedropper
Zoom
Size
Hardness
Opacity
BlendMode
Normal
Multiply
Screen
Layers
Adjust
History
Brightness
Contrast
Saturation
Temperature
ColorModeRgb
BitDepth8
```

Rules:

- zh-TW is default.
- English translations must also exist.
- Do not hard-code new UI labels in only one language unless it is a temporary internal debug label.

---

## 13. Preserve and migrate existing features

Before deleting or hiding legacy UI, confirm these remain reachable:

- open single image
- open folder / browse images
- drag-and-drop open
- clipboard paste
- zoom / fit / 100%
- rotate / flip / resize / crop / selection
- brightness / contrast / saturation / exposure / blur / sharpen
- grayscale / invert / auto contrast
- hue / sepia / vignette / temperature / tint / posterize
- text/image layers and watermark
- layer order / duplicate / hide
- export composite
- save/open `.pixel-project`
- convert formats
- batch conversion
- GIF generation
- 360 GPano export
- C2PA manifest view / add / remove / batch remove
- zh-TW / English switch

If a feature cannot be migrated in one pass, keep it accessible through a compact legacy drawer or menu command and add a TODO comment.

---

## 14. Refactor strategy for `app.rs`

Because `app.rs` is large, use this order:

1. Add new `ui` module files with minimal compile-safe functions.
2. Add `UiState` fields and defaults.
3. Implement `theme.rs` and apply visuals.
4. Implement TopMenuBar + StatusBar first because they need minimal existing state.
5. Implement ToolRail and shortcuts.
6. Implement RightPanel tabs with existing data adapters.
7. Move existing preview rendering into `CanvasView` last.
8. Remove duplicated legacy UI only after equivalent new UI exists.

Keep each phase compiling.

---

## 15. Validation commands

Run these from repo root:

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p studio-app
```

If formatting changed:

```powershell
cargo fmt
```

Do not claim completion unless all checks pass or the failure is documented with exact error output and next fix.

---

## 16. Manual visual QA checklist

After `cargo run -p studio-app`, verify:

- [ ] Window opens around the existing 1440×960 default size and respects minimum size.
- [ ] TopMenuBar height visually matches 40px.
- [ ] ContextualToolbar height visually matches 34px.
- [ ] ToolRail width visually matches 50px.
- [ ] RightPanel width visually matches 260px.
- [ ] StatusBar height visually matches 26px.
- [ ] UI is flat: no gradient, no heavy shadow.
- [ ] Borders are thin and subtle.
- [ ] Active tool uses blue-tinted selected state.
- [ ] Active right-panel tab has blue underline.
- [ ] Canvas background is tertiary gray.
- [ ] Zoom badge appears at bottom-right of canvas.
- [ ] Zoom slider changes CanvasView zoom.
- [ ] Dark mode uses dark tokens and remains readable.
- [ ] zh-TW labels display by default.
- [ ] English switch still works if existing language selector remains.
- [ ] No text clips in the 260px RightPanel.

---

## 17. Definition of done

This task is done when:

- The app uses the new four-section + three-column editor layout.
- The visual language matches the provided UI prototype closely in egui.
- New UI code is modular under `crates/studio-app/src/ui/`.
- Existing core image processing behavior remains intact.
- New labels are represented in `i18n.rs`.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass.
- Any intentionally deferred feature migration is listed in a short `Follow-up TODO` section in the final Codex response.

---

## 18. Suggested first Codex prompt

```text
Read AGENTS.md, README.md, UI_DESIGN_SPEC.md, ui_prototype.html, and the current crates/studio-app/src/app.rs. Then implement the TODOs in todos.md phase by phase. Keep the app Rust + egui. Do not rewrite in React. First create the ui module structure and theme tokens, then recompose the main app layout into TopMenuBar, ContextualToolbar, ToolRail, CanvasView, RightPanel, and StatusBar. Preserve existing image editing features and run cargo fmt/clippy/test after each major phase.
```
