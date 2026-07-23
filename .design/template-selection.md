# Pixel Studio Template Selection

- Mode: redesign
- Reference template: `media-studio`
- Visual recipe: `focused-productivity` (bright)
- Target platform: Windows desktop
- Runtime route: AcmeUI-Native design architecture mapped to the existing Rust UI runtime

## Runtime decision

AcmeUI-Native was evaluated from the official repository at commit
`b1f12e6447b6a5ecb4520ba42c49ba1cdea2430b`. Its current desktop `ImageView` is a placeholder
and `acme-core::DrawCommand` exposes quads and text but no image texture command. Moving Pixel
Studio directly to that runtime now would remove the real image canvas and clipboard-image
workflow.

The redesign therefore follows the skill's supported “other Rust UI stack” route: preserve the
working editor runtime while applying AcmeUI's media-studio regions, semantic tokens, explicit
states and visual-QA gates. This is a capability decision, not a dependency preference.

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
