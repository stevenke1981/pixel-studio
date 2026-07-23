# Pixel Studio Component Inventory

| Component | Responsibility | States |
| --- | --- | --- |
| Command bar | Capture/open/paste/save, undo/redo, workspace, zoom | enabled, disabled, selected |
| Tool rail | Select, crop, text, line, arrow, rectangle, hand | idle, selected, hover |
| Canvas | Empty-state launch actions or editable image | empty, loaded, selecting, panning |
| Inspector | Quick workflow, privacy/export, crop and layers | no-image, image, selection, layer |
| Advanced drawer | Project, conversion, filters, compose, panorama, GIF, batch | collapsed, expanded |
| Status bar | Operation status, image dimensions, zoom, selection size | idle, active |

The inventory deliberately reuses current egui controls and application state. No duplicate tool
model or parallel UI framework is added.
