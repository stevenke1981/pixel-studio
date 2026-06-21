# Failure log

## 2026-06-21 — Editor UI redesign

| Attempt | Failure | Resolution |
|---|---|---|
| Cargo default target | Windows denied writes to an existing incremental cache (`os error 5`). | Reused an isolated `CARGO_TARGET_DIR` under `%TEMP%` for verification. |
| Workspace scratch target | Cargo could not create the alternate directory inside the workspace sandbox. | Moved the target directory to `%TEMP%`; all checks then ran normally. |
| Default WGPU backend | Process stayed alive but did not create a usable window in the QA environment. | Default `WGPU_BACKEND` to `gl` before thread creation while preserving explicit user overrides. |
| Desktop screenshot APIs | `CopyFromScreen` failed and `PrintWindow` returned a blank GPU surface. | Used egui's own screenshot viewport command for the verified frame. |
| CBM ADR store | The MCP call did not return within 60 seconds. | Kept the same architecture decision in `.opencode/plans/ui-redesign.md`; no code action depended on the service response. |

## 2026-06-21 — Left tool category migration

| Attempt | Failure | Resolution |
|---|---|---|
| Delete obsolete UI module with `apply_patch` | Windows returned a delete failure for `ui/tool_rail.rs`. | Used an approved targeted `git rm` after `left_tools.rs` compiled successfully. |
