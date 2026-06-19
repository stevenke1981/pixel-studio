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
