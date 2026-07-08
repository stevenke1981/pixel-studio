# Common Toolbar Plan

## Goal

Make the most-used Pixel Studio actions directly visible in the top toolbar while keeping the left categorized panel as the complete workflow library.

## Scope

- Generate and save a GUI reference image for the toolbar direction.
- Add compact top-toolbar buttons for frequent file, history, tool, zoom, and export actions.
- Preserve the existing menu bar, contextual toolbar, left category rail, right panel, and canvas behavior.

## Implementation

1. Save the generated GUI reference at `docs/assets/pixel-studio-toolbar-reference.png`.
2. Extend `top_menu_bar::show_top_menu_bar` to receive `active_tool`.
3. Add icon-sized toolbar buttons for:
   - open image
   - paste image
   - save project
   - save as
   - undo / redo
   - crop / brush / line tool selection
   - fit to window
   - 100%
   - export
4. Style selected tool buttons with the existing blue info token.
5. Keep full feature access in the left categorized tools panel and menus.

## Validation

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo build --release --workspace`
- GUI smoke test from the release executable.
