---
## Lesson #1 - 2026-06-19
**Trigger:** A zero-length brush stroke was stamped twice because the interpolation loop treated a dot as a one-step segment.
**Rule:** Before accepting canvas drawing code, add or run a unit test for zero-length pointer strokes so single clicks do not compound opacity.
**Source:** drawing tools
