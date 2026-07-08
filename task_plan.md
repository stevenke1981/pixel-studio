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

---

# Task Plan: App Icons and Snapshottable Trait

## Goal

Add Pixel Studio app icons/ICO assets, wire them into the GUI and Windows release build, extract `CanvasSnapshot` app access behind a `Snapshottable` trait, validate release, then commit and push.

## Current Phase

Phase 1

## Phases

### Phase 1: Discovery
- [x] Check git status and create task branch.
- [x] Locate `CanvasSnapshot`, clear/restore paths, `main.rs`, and Cargo config.
- [x] Inspect icon/resource options and existing assets.
- **Status:** complete

### Phase 2: Icon Assets
- [x] Generate Pixel Studio icon image using built-in image generation.
- [x] Convert and save PNG/ICO app icon assets under project resources.
- [x] Wire runtime window icon and Windows executable resource icon.
- **Status:** complete

### Phase 3: Snapshottable Refactor
- [x] Add `Snapshottable` trait.
- [x] Move `CanvasSnapshot` capture/restore behind trait methods.
- [x] Update clear/restore paths to use the trait boundary.
- **Status:** complete

### Phase 4: Validation
- [x] Run cargo fmt --check.
- [x] Run cargo clippy --workspace --all-targets -- -D warnings.
- [x] Run cargo test --workspace.
- [x] Build release workspace and copy `dist` binaries.
- [x] Smoke-test release GUI.
- **Status:** complete

### Phase 5: Delivery
- [x] Update docs/todos/lessons/progress.
- [x] Commit and push.
- [x] Verify remote SHA matches local HEAD.
- **Status:** complete

## Decisions Made

| Decision | Rationale |
|----------|-----------|
| Use built-in image generation for the icon art and local conversion for ICO | No separate ico-generation skill is available in this environment. |
| Keep `CanvasSnapshot` as the concrete data object and add `Snapshottable` for app boundary methods | This satisfies the requested abstraction without forcing a broad app-state rewrite. |
