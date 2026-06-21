## Plan: Categorize legacy features in the left tool area
**Goal:** Move every legacy tool-window workflow into a categorized left rail and drawer, then ship a verified Windows release build.
**Complexity:** L4

### Functional specification
- Replace the mode-only rail with nine visible feature categories: File, Browse, Drawing, Edit, Layers, Export, GIF, Batch, and C2PA.
- Keep a 50px category rail plus a 300px scrollable drawer; allow the drawer to be hidden from View without removing access to the category rail.
- Default to Drawing so Brush and Line remain visible immediately.
- Reuse existing `PixelStudioApp` behavior methods and panel controls; do not duplicate image-processing logic.
- Remove the legacy floating window after every contained workflow is reachable in the left drawer.

### Function inventory
| Function | Kind | Responsibility | Inputs | Output / edge cases |
|---|---|---|---|---|
| `LeftToolCategory::all/icon/text_key` | new | Define stable category order and bilingual presentation | category | Static metadata |
| `show_category_rail` | new | Draw category buttons and update active category | UI, tokens, language, state | Category selection |
| `show_tool_picker` | new | Draw direct canvas mode buttons and keyboard shortcuts | context, UI, state | Typed `SelectTool` actions |
| `show_left_category_panel` | new adapter | Route the selected category to existing app panel functions | UI, context, category | Existing controls rendered once |
| `file_tools_panel` | new adapter | Render open/save/project/folder/navigation/language commands | UI, context | Existing file methods invoked |
| `metadata_panel` | existing, adjusted | Render image/folder browser without embedded C2PA duplication | UI, context | Existing browse behavior |
| `edit_panel`, `compose_panel`, `convert_panel`, `panorama_panel`, `gif_panel`, `batch_panel`, `c2pa_panel` | existing, referenced | Render their current workflows in one category | Existing app state | No behavior contract change |

### Architecture
```text
Top bars
Workspace
  -> 50px category rail
  -> optional 300px category drawer
       -> file / browse / drawing / edit / layers / export / GIF / batch / C2PA
       -> existing PixelStudioApp methods
  -> canvas
  -> right layers/adjust/history panel
Status bar
```

### Sub-tasks
1. [x] Add category state, rail metadata, picker, and tests.
2. [x] Add the left drawer and route all nine categories.
3. [x] Extract file controls and remove C2PA duplication from Browse.
4. [x] Remove the legacy window/action aggregator and update View toggle/i18n.
5. [x] Run formatting, clippy, tests, screenshot QA, and release build.
6. [ ] Record lessons, commit, push, and verify remote SHA.

### Risks
| Risk | Mitigation |
|---|---|
| A legacy action becomes unreachable | Inventory every legacy panel and map it to exactly one category before deleting the window |
| Left drawer crowds the canvas | Fixed 300px width, hide toggle, existing centered/fit canvas behavior |
| Tool shortcuts conflict with commands | Ignore Ctrl/Command/Alt combinations and process text input focus first |
| Duplicate C2PA controls remain | Remove the embedded C2PA call from Browse; keep all C2PA UI in one category |

### Definition of Done
- [x] Every legacy workflow is reachable from a named left category.
- [x] Brush and Line remain visible under the default Drawing category.
- [x] Legacy floating window and duplicate aggregator are removed.
- [x] `cargo fmt --check`, strict clippy, and workspace tests pass.
- [x] Native screenshot QA shows no overlap at the default viewport.
- [x] Release workspace build succeeds and release binaries are identified.
- [ ] Git commit and push complete.

### Assumptions
- “左側工具列 分類別” means a compact category rail plus adjacent categorized drawer, not placing every control into 50px.
- Existing right Layers/Adjust/History panel remains because it is part of the approved editor layout.
