# Task Plan: Pixel Studio UI Reference Alignment and OOP Refactor

## Goal

Align the Rust egui app UI with the generated toolbar reference image, introduce a modest OOP-style UI presentation structure, build the release binaries, and push the completed work.

## Current Phase

Phase 5

## Phases

### Phase 1: Requirements & Discovery
- [x] Capture user requirements.
- [x] Inspect branch state and generated reference image.
- [x] Locate current UI module boundaries and app.rs hotspots.
- **Status:** complete

### Phase 2: Design & Refactor Plan
- [x] Define which UI elements must change to match the generated image.
- [x] Define OOP-style structs/traits or presenter modules without broad behavior changes.
- [x] Document decisions in findings.md.
- **Status:** complete

### Phase 3: Implementation
- [x] Add reference-aligned top common toolbar row.
- [x] Adjust contextual toolbar and left tools to visually follow the reference.
- [x] Move toolbar presentation into an object-style component structure.
- [x] Preserve existing editor behavior.
- **Status:** complete

### Phase 4: Validation
- [x] Run cargo fmt --check.
- [x] Run cargo clippy --workspace --all-targets -- -D warnings.
- [x] Run cargo test --workspace.
- [x] Build release workspace and update dist binaries.
- [x] Smoke-test release GUI.
- **Status:** complete

### Phase 5: Delivery
- [x] Update README/todos/lessons/progress with validation evidence.
- [x] Commit and push.
- [x] Verify remote SHA matches local HEAD.
- **Status:** complete

## Key Questions

1. How far can the UI be aligned to the generated image without replacing egui with a new framework?
2. Which OOP-style abstraction reduces app.rs/UI-module coupling while staying small and behavior-preserving?
3. How can visual changes be validated in this native egui environment?

## Decisions Made

| Decision | Rationale |
|----------|-----------|
| Use the generated image as a directional reference, not a pixel-perfect external design system | The app is Rust egui and must preserve existing workflows. |
| Keep OOP refactor scoped to UI presentation/controller structs | Large app.rs behavior rewrites would risk image editing regressions. |
| Use `TopChrome` and `ContextualToolbar` presenter structs | Adds OOP design concepts while preserving existing `UiAction` dispatch. |

## Errors Encountered

| Error | Attempt | Resolution |
|-------|---------|------------|
| planning-with-files skill missing under `.agents` path | 1 | Read the Codex skill copy from `C:\Users\eda\.codex\skills\planning-with-files`. |

## Notes

- Default response language: zh-TW.
- Do not stage `.codebase-memory/` or `.opencode` runtime cache files.
