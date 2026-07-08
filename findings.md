# Findings & Decisions

## Requirements

- Modify the app UI based on the generated GUI reference image.
- Make the UI visually match the image more closely.
- Add OOP design concepts and reorganize app code.
- Compile release binaries.
- Commit and push the result.

## Research Findings

- Current branch starts from `codex/left-tool-categories` with only runtime cache files untracked.
- Generated reference image is saved at `docs/assets/pixel-studio-toolbar-reference.png`.
- The reference image shows a clearer hierarchy than the current app:
  - first row: app title plus menus;
  - second row: large common action toolbar with icon above label;
  - third row: contextual brush controls;
  - left side: slim vertical named tool rail plus an open drawing tools drawer;
  - center: gray canvas area;
  - right side: layer/adjust/history tabs;
  - bottom: slim status bar with dimensions and zoom controls.
- Current code still renders menu and compact shortcut buttons together inside one 40px `top_menu_bar` panel.
- Current contextual toolbar is already a separate row, so it can be retained and visually widened to follow the reference.

## Technical Decisions

| Decision | Rationale |
|----------|-----------|
| Add a dedicated common toolbar row below the menu row | The generated image clearly separates menu navigation from frequent actions. |
| Use object-style presenter structs for toolbar rendering | Adds OOP design concepts without rewriting the entire editor state machine. |
| Keep core image operations dispatched through existing `UiAction` | Preserves undo/history and current app behavior. |
| Split top chrome into menu row + common toolbar row + contextual row | Matches the generated image and keeps frequent actions visible. |

## Issues Encountered

| Issue | Resolution |
|-------|------------|
| Superpowers workflow tools are not directly callable in this session | Use available local skills and the Controlled Workflow fallback from AGENTS instructions. |

## Resources

- `docs/assets/pixel-studio-toolbar-reference.png`
- `crates/studio-app/src/app.rs`
- `crates/studio-app/src/ui/top_menu_bar.rs`
- `crates/studio-app/src/ui/contextual_toolbar.rs`
- `crates/studio-app/src/ui/left_tools.rs`

## Visual/Browser Findings

- The reference toolbar uses larger 64-72px wide action cells with icon and label, separated into groups: open/paste, save/save as, undo/redo, crop/brush/line, fit/100%, export.
- The brush tool is visibly active with blue accent fill, matching the app's existing `bg_info` / `text_info` token intent.
- The contextual toolbar is a separate thin row with brush size, opacity, color, and blend mode controls.

---

# Findings: App Icons and Snapshottable Trait

## Requirements

- Inspect the project before changes.
- Use available icon/image generation workflow to add app icons/ICO.
- Execute `CanvasSnapshot → Snapshottable trait` refactor because `CanvasSnapshot` still directly operates on the app struct.
- Build release binaries.
- Commit and push with remote confirmation.

## Research Findings

- `CanvasSnapshot` is currently defined in `crates/studio-app/src/app.rs`.
- `clear_canvas()` stores `self.cleared_canvas = Some(self.capture_canvas_snapshot())`.
- `restore_canvas()` manually copies every snapshot field back into `PixelStudioApp`.
- `capture_canvas_snapshot()` clones app fields directly into `CanvasSnapshot`.
- `main.rs` creates `eframe::NativeOptions` with title/size only; no runtime icon is configured.
- `crates/studio-app/Cargo.toml` has no build script or Windows resource dependency yet.
- `egui::ViewportBuilder::with_icon` accepts an `egui::IconData`.
- `eframe::icon_data::from_png_bytes` can load an embedded PNG into `IconData`.
- `winresource` provides `WindowsResource::set_icon(...).compile()` for a Windows `.exe` resource icon.
- The icon generator skill created `assets/icon/pack/windows/pixel-studio.ico` with 16, 24, 32, 48, 64, 128, and 256 px entries.

## Technical Decisions

| Decision | Rationale |
|----------|-----------|
| Generate a square icon PNG first, then derive `.ico` from it locally | Keeps the generated art inspectable and lets Windows resource use a standard ICO. |
| Add `build.rs` only to `studio-app` | The icon is app-specific and should not affect core/CLI crates. |
| Add a `Snapshottable` trait implemented for `PixelStudioApp` | Separates snapshot capture/restore behavior from callers while preserving existing data fields. |
| Embed `assets/icon/pack/png/256x256.png` for runtime icon data | Keeps startup self-contained without loading an external file. |
