# Pixel Studio Component Inventory

| Component | Responsibility | States |
| --- | --- | --- |
| Command bar | Capture/open/paste/save, undo/redo, workspace, zoom | enabled, disabled, selected |
| Tool rail | Select, crop, text, line, arrow, rectangle, hand | idle, selected, hover, focus |
| Canvas | Empty-state launch actions or editable image | empty, loaded, selecting, panning |
| Inspector | Quick workflow, privacy/export, crop and layers | no-image, image, selection, layer |
| Privacy group | Select region, blur, remove C2PA, export WebP | disabled, ready, selected |
| Advanced drawer | Project, conversion, filters, compose, panorama, GIF, batch | collapsed, expanded |
| Status bar | Operation status, image dimensions, zoom, selection size | idle, active |

## Semantic state mapping

| State | Surface | Border | Text |
| --- | --- | --- | --- |
| Default | `SURFACE_ALT` | `BORDER` | `TEXT` |
| Hover | `ACCENT` | `PRIMARY` | `PRIMARY_DARK` |
| Selected | `ACCENT` | `PRIMARY` | `PRIMARY_DARK` |
| Pressed | `PRIMARY` | `PRIMARY_DARK` | `PRIMARY_TEXT` |
| Disabled | default at 62% opacity | default at 62% opacity | `TEXT` at 62% opacity |

The inventory reuses the current application state and editor engine. No duplicate tool model or
parallel UI framework is introduced.
