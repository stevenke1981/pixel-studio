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
---
## Lesson #6 - 2026-07-08
**Trigger:** Common actions were reachable through menus and the left drawer, but still not visible enough as first-level controls.
**Rule:** When a workflow is described as common or frequent, expose it in the top toolbar with a compact button and tooltip while keeping the full categorized panel for deeper tools.
**Source:** common toolbar migration
---
## Lesson #7 - 2026-07-08
**Trigger:** The generated GUI reference had separate menu, action, and contextual rows, while the app initially squeezed frequent actions into the menu bar.
**Rule:** When matching a visual reference, preserve the reference's row hierarchy first, then tune button styling; do not compress distinct rows into one control strip.
**Source:** reference UI and OOP refactor
---
## Lesson #8 - 2026-07-08
**Trigger:** App icon work needed both runtime window icon wiring and a Windows executable resource icon.
**Rule:** For Windows Rust egui apps, embed a PNG with `ViewportBuilder::with_icon` for runtime windows and add a crate-local `build.rs` with `winresource` for the `.exe` icon.
**Source:** app icons and Snapshottable refactor
---
## Lesson #9 - 2026-07-08
**Trigger:** `CanvasSnapshot` capture and restore were directly called as app helper methods from clear/restore flows.
**Rule:** Put snapshot capture/restore behind a small trait and add a round-trip test before changing clear/restore behavior.
**Source:** CanvasSnapshot to Snapshottable trait
