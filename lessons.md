---
## Lesson #1 - 2026-06-19
**Trigger:** A zero-length brush stroke was stamped twice because the interpolation loop treated a dot as a one-step segment.
**Rule:** Before accepting canvas drawing code, add or run a unit test for zero-length pointer strokes so single clicks do not compound opacity.
**Source:** drawing tools
---
## Lesson #2 - 2026-06-19
**Trigger:** Drawing controls were implemented inside the Edit panel but were not visible enough in the app UI.
**Rule:** When adding a new interactive GUI tool, expose its mode switch in the top toolbar or first visible action-panel section before considering the feature delivered.
**Source:** drawing tools visibility fix
---
## Lesson #3 - 2026-06-21
**Trigger:** Registering every file in `C:\Windows\Fonts` with egui before the first frame consumed about 1.4 GB and prevented timely window creation.
**Rule:** Register only the required UI fallback font at startup; load user-selected document fonts by path when rendering content instead of preloading the whole system font directory.
**Source:** editor UI redesign visual QA
---
## Lesson #4 - 2026-06-21
**Trigger:** `CopyFromScreen` and `PrintWindow` could not capture the GPU-rendered egui surface even though the window was alive.
**Rule:** For native egui visual QA, request `ViewportCommand::Screenshot` and validate the returned `Event::Screenshot` image instead of relying on desktop capture APIs.
**Source:** editor UI redesign visual QA
---
## Lesson #5 - 2026-06-21
**Trigger:** The old drawing panel edited engine fields directly, but the redesigned UI synchronizes those fields from `UiState` at the end of every frame.
**Rule:** When migrating controls into the categorized left panel, edit `UiState` for synchronized tools instead of reusing controls that write the derived engine fields.
**Source:** legacy tools to left categories migration
