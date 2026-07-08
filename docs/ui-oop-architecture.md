# Pixel Studio UI OOP Notes

## Goal

Pixel Studio remains a Rust + egui application, but the top UI chrome now uses object-style presenter structs so layout rendering is no longer just a flat list of free functions.

## Current Pattern

- `TopChrome` owns the menu row and common action toolbar presentation.
- `ContextualToolbar` owns active-tool-specific controls.
- `UiState` remains the UI state object for selected tools, brush settings, panel tabs, and zoom.
- `UiAction` remains the command boundary between UI components and `PixelStudioApp`.

## Why This Shape

This keeps the editor's core behavior in `PixelStudioApp` while moving view composition into smaller presenter objects. It is an OOP-style structure without forcing Rust into inheritance-heavy design.

## Reference Alignment

The generated GUI reference image is stored at:

```text
docs/assets/pixel-studio-toolbar-reference.png
```

The app now follows its visible hierarchy:

1. menu/title row;
2. large common action toolbar;
3. contextual tool settings row;
4. left labeled tool/category rail with drawer;
5. center canvas;
6. right inspector tabs;
7. bottom status bar.
