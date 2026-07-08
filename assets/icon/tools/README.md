# Pixel Studio Tool Icons

This folder contains app-internal toolbar and panel icons. They are separate from
the executable/app icon pack in `assets/icon/pack`.

- `reference-sheet.png` is the Codex-generated visual reference used for the
  internal tool icon style.
- `contact-sheet.png` is a local review sheet for the generated PNG assets.
- Individual `*.png` files are embedded by `crates/studio-app/src/ui/tool_icons.rs`
  with `egui::include_image!`.

The icons are designed for the common toolbar, left category rail, and drawing
tool picker at roughly 18-20 px in the egui UI.
