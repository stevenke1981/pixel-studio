# Pixel Studio Template Selection

- Mode: redesign
- Reference template: `media-studio`
- Visual recipe: `studio-neutral`
- Runtime decision: keep the existing Rust/egui application.

The AcmeUI media-studio template is used as an information-architecture reference only.
Pixel Studio is not an AcmeUI-Native application, so AcmeUI `WidgetNode`, shell, and theme-pack
APIs are intentionally not introduced.

## Region mapping

| Template region | Pixel Studio region |
| --- | --- |
| Command bar | Capture, open, paste, save, undo/redo, workspace and zoom |
| Asset rail | Focused vertical annotation tool rail |
| Preview | Image canvas |
| Inspector | Quick workflow, privacy, crop, layers and advanced tools |
| Timeline/status | Compact status bar with dimensions, zoom and selection |

## Product state

- No image: prominent capture/open/paste starting actions.
- Image loaded: tool rail, canvas and inspector operate as one workflow.
- Selection active: crop and regional blur actions become available.
- Export: PNG/JPEG/WebP remain available through Save Copy.
