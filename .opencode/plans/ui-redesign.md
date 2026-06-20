## Plan: Pixel Studio editor UI redesign
**Goal:** Recompose the Rust + egui app into the compact four-band, three-column editor described by `todos.md` while preserving every existing workflow.
**Complexity:** L4

### Functional specification
- Render fixed-height top menu, contextual toolbar, and status bar around a left tool rail, center canvas, and right tab panel.
- Keep image operations in `PixelStudioApp` and `studio-core`; UI modules emit actions or edit presentation state only.
- Keep all legacy workflows reachable from menus or a compact legacy tools window during migration.
- Synchronize brush and zoom UI state with the existing drawing and preview engines.
- Use bilingual `TextKey` labels and semantic light/dark theme tokens.

### Function inventory
| Function | Kind | Responsibility | Inputs | Output / edge cases |
|---|---|---|---|---|
| `UiTokens::from_ctx` | new | Resolve semantic colors for light/dark mode | egui context | Complete token set |
| `apply_visuals` | new | Apply flat spacing, rounding, strokes, and colors | egui context, tokens | Mutates global egui style |
| `UiState::default` | new | Initialize tool, tab, brush, adjust, zoom, and legacy state | none | Valid clamped defaults |
| `show_top_menu_bar` | new | Draw menus and command buttons | UI, tokens, language, command availability | Optional `UiAction` |
| `show_contextual_toolbar` | new | Draw settings for active tool | UI, tokens, language, mutable UI state | Updated settings/action |
| `show_tool_rail` | new | Draw tool modes and process shortcuts | context/UI/state | Active tool change |
| `show_right_panel` | new | Draw Layers/Adjust/History tabs | UI, tokens, layer adapter data, UI state | Layer/adjust actions |
| `show_status_bar` | new | Draw dimensions and zoom controls | UI, tokens, dimensions, UI state | Updated zoom |
| `show_canvas_view` | new | Allocate and paint centered image surface and zoom badge | UI, tokens, texture geometry | Canvas response/rect/scale |
| `dispatch_ui_action` | new adapter | Route UI commands to existing app methods | context, `UiAction` | Existing behavior invoked safely |
| Existing open/save/export/undo/filter/layer methods | reference | Preserve current business behavior | existing app state | No contract change |

### Architecture
```text
eframe::App::ui
  -> UiTokens + UiState
  -> ui::{top menu, contextual toolbar, tool rail, right panel, status bar}
       -> UiAction
       -> PixelStudioApp adapters -> existing app/core methods
  -> ui::canvas_view allocation
       -> existing canvas interaction and painting methods
  -> optional legacy tools window -> existing panels
```

### Sub-tasks
1. [x] Add `ui/` module, semantic theme, UI state, shared actions and view models.
2. [x] Add top menu, contextual toolbar, tool rail, and status bar.
3. [x] Add right tabs with real compose-layer adapters and adjustment controls.
4. [x] Move canvas allocation/texture painting into `CanvasView` and preserve interactions.
5. [x] Recompose the app layout and keep all legacy functions reachable.
6. [ ] Add all zh-TW/English labels, tests, visual smoke verification, and Git delivery.

### Risks
| Risk | Mitigation |
|---|---|
| Large `app.rs` refactor leaves duplicate or stale UI | Compile after each phase; run fmt/clippy immediately after layout replacement |
| UI modules cannot access private app state | Pass view models and return typed actions instead of widening all field visibility |
| Adjustment sliders repeatedly destroy image quality | Treat new values as UI state and apply only on explicit commit |
| New layout hides specialist workflows | Keep a visible legacy tools command/window until every workflow is migrated |
| Brush/zoom state diverges | Synchronize UI state to existing engine fields at adapter boundaries |

### Definition of Done
- [x] New modular layout and semantic theme match the reference structure.
- [x] Drawing tools remain first-viewport visible and functional.
- [x] Existing image, project, layers, conversion, batch, GIF, panorama, and C2PA workflows remain reachable.
- [x] `cargo fmt --check`, clippy, and workspace tests pass.
- [x] GUI launches successfully and manual visual checks are recorded.
- [ ] Git commit and push complete.

### Assumptions
- `todos.md` and the explicit instruction to start implementation constitute blueprint approval.
- Specialist legacy panels may remain in a compact tools window for this migration pass.
- Brush hardness and blend mode are presentation state until the raster engine gains matching behavior.
