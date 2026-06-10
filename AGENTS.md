# Pixel Studio — Agent Notes

Rust workspace at `D:\pixel-studio` — image editor forked from photo-tool with extended filters and modular i18n.

## Workspace layout

- `crates/studio-core/` — image processing, format conversion, batch, GIF, 360 panorama + GPano/XMP, C2PA, compose layers, **filters** (hue, sepia, vignette, temperature, tint, auto contrast, posterize). No UI dependencies.
- `crates/studio-app/` — egui desktop app. Depends on `studio-core`. UI strings in `src/i18n.rs` (`TextKey` + `Language::ZhTw` / `Language::En`).
- `crates/studio-cli/` — clap-based CLI. Mirrors GUI features for smoke testing.

`Cargo.toml` at repo root is a workspace; do not add a `[package]` there.

## Build & verify

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`cargo run -p studio-app` launches the GUI. `cargo run -p studio-cli -- <subcommand>` runs the CLI.

## i18n

- Default language: Traditional Chinese (`Language::ZhTw`).
- Language switch in the File panel (`TextKey::Language`).
- When adding user-facing labels: add a `TextKey` variant plus `zh_tw()` and `en()` arms in `crates/studio-app/src/i18n.rs`.

## Project files

Editable projects use `.pixel-project` (JSON with base64 PNG payloads). Legacy `.photo-project` files from photo-tool are not auto-migrated.

## Extended filters (`crates/studio-core/src/filters.rs`)

- `rotate_180`, `adjust_hue`, `sepia`, `vignette`, `adjust_temperature`, `adjust_tint`, `auto_contrast`, `posterize`
- GUI controls live in `edit_panel` in `app.rs` under the "進階濾鏡 / Advanced Filters" section.

## Gotchas

- Same dependency pins as photo-tool (`image` limited decoders, `c2pa` with `rust_native_crypto` only).
- `app.rs` is large. Use grep on `TextKey::` / `fn edit_panel` / `fn batch_panel` to navigate.
- Batch C2PA removal is a standalone tool (not a `BatchOptions` flag), same as photo-tool.