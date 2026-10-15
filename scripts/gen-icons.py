#!/usr/bin/env python3
"""Generate Pixel Studio's original SVG icon set (24x24, 1.8px round strokes).

Icons are monochrome black; the UI recolours them with Slint's `colorize`.
Run: python3 scripts/gen-icons.py  (writes crates/studio-app/ui/icons/*.svg)
"""
import os

OUT = os.path.join(os.path.dirname(__file__), "..", "crates", "studio-app", "ui", "icons")
HEAD = ('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" '
        'fill="none" stroke="#000" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">')
F = 'fill="#000" stroke="none"'  # solid fill helper

doc = '<path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4"/>'
folder = '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>'
disk = '<path d="M5 4h11l3 3v12a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1z"/><path d="M8 4v5h7V4"/><rect x="8" y="13" width="8" height="5"/>'
glass = '<circle cx="10.5" cy="10.5" r="6"/><path d="M15 15l5.5 5.5"/>'
frame = '<rect x="3" y="5" width="18" height="14" rx="2"/>'
mountain = '<circle cx="9" cy="10" r="1.6"/><path d="M21 16l-5-5-9 8"/>'
layers = '<path d="M12 4l9 4.5-9 4.5-9-4.5z"/><path d="M3 13l9 4.5 9-4.5"/>'
clip = '<rect x="6" y="4" width="12" height="17" rx="2"/><rect x="9" y="2.5" width="6" height="3" rx="1"/>'

I = {
 # ---- file
 "new": doc + '<path d="M12 11v6M9 14h6"/>',
 "open": folder + '<path d="M12 10v6M9.5 12.5L12 10l2.5 2.5"/>',
 "folder": folder,
 "save": disk,
 "save-as": '<path d="M5 4h9l3 3v4"/><path d="M4 5v14a1 1 0 0 0 1 1h6"/><path d="M8 4v4h5V4"/><path d="M14 20l1-3 5-5 2 2-5 5z"/>',
 "project-open": layers + '<path d="M17 18h4M19 16v4"/>',
 "project-save": layers.replace('M3 13l9 4.5 9-4.5','M3 13l9 4.5 3-1.5') + '<path d="M17 15v6M14.5 18.5L17 21l2.5-2.5"/>',
 "export": '<path d="M14 4h6v6M20 4l-9 9"/><path d="M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/>',
 "print": '<path d="M7 9V3h10v6"/><rect x="3" y="9" width="18" height="8" rx="2"/><path d="M7 14h10v7H7z"/>',
 "properties": doc + '<path d="M9 12h6M9 15h6M9 18h4"/>',
 "exit": '<path d="M10 4H5a1 1 0 0 0-1 1v14a1 1 0 0 0 1 1h5"/><path d="M15 8l4 4-4 4M19 12H9"/>',
 "recent": '<circle cx="12" cy="12" r="8.5"/><path d="M12 7v5l3 2"/>',
 # ---- edit
 "undo": '<path d="M9 14L4 9l5-5"/><path d="M4 9h11a5 5 0 0 1 0 10h-3"/>',
 "redo": '<path d="M15 14l5-5-5-5"/><path d="M20 9H9a5 5 0 0 0 0 10h3"/>',
 "cut": '<circle cx="6" cy="17" r="3"/><circle cx="18" cy="17" r="3"/><path d="M8.2 15L18 3M15.8 15L6 3"/>',
 "copy": '<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3"/>',
 "paste": clip + '<path d="M9 11h6M9 15h4"/>',
 "paste-layer": clip.replace('height="17"','height="11"') + '<path d="M8 17.5l4 2 4-2"/><path d="M8 20.5l4 2 4-2"/>',
 "paste-new": clip + '<path d="M12 10v7M8.5 13.5h7"/>',
 "select-all": '<path d="M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3"/><path d="M11 4h2M11 20h2M4 11v2M20 11v2"/><rect x="8" y="8" width="8" height="8" ' + F + ' opacity=".35"/>',
 "deselect": '<path d="M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3"/><path d="M9 9l6 6M15 9l-6 6"/>',
 "delete": '<path d="M4 7h16M10 3h4M6 7l1 13h10l1-13"/><path d="M10 11v6M14 11v6"/>',
 "screenshot": '<path d="M4 8V6a2 2 0 0 1 2-2h2M16 4h2a2 2 0 0 1 2 2v2M20 16v2a2 2 0 0 1-2 2h-2M8 20H6a2 2 0 0 1-2-2v-2"/><circle cx="12" cy="12" r="3.2"/>',
 # ---- view
 "zoom-in": glass + '<path d="M8 10.5h5M10.5 8v5"/>',
 "zoom-out": glass + '<path d="M8 10.5h5"/>',
 "zoom-fit": '<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/><rect x="8" y="8" width="8" height="8" rx="1"/>',
 "zoom-100": '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M7 10l1.5-1v6M15 10l1.5-1v6"/><circle cx="12" cy="10.5" r=".5" ' + F + '/><circle cx="12" cy="13.5" r=".5" ' + F + '/>',
 "grid": '<rect x="4" y="4" width="16" height="16" rx="1"/><path d="M9.33 4v16M14.67 4v16M4 9.33h16M4 14.67h16"/>',
 "ruler": '<path d="M3 16L16 3l5 5L8 21z"/><path d="M7 12l2 2M10 9l2 2M13 6l2 2"/>',
 "toolbar": '<rect x="3" y="5" width="18" height="14" rx="2"/><rect x="6" y="8" width="12" height="3" rx="1"/>',
 "statusbar": '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 15h18M6 17h4"/>',
 "fullscreen": '<path d="M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5"/>',
 "mode-view": '<path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z"/><circle cx="12" cy="12" r="3"/>',
 "mode-edit": '<path d="M4 20l1-4L16 5l3 3L8 19z"/><path d="M14 7l3 3"/>',
 # ---- image
 "crop": '<path d="M6 2v14a2 2 0 0 0 2 2h14"/><path d="M2 6h14a2 2 0 0 1 2 2v14"/>',
 "resize": '<rect x="3" y="11" width="10" height="10" rx="1"/><path d="M14 4h6v6M20 4l-7 7"/>',
 "canvas-size": '<rect x="7" y="7" width="10" height="10" rx="1"/><path d="M3 7V3h4M17 3h4v4M21 17v4h-4M7 21H3v-4"/>',
 "rot-left": '<path d="M4 12a8 8 0 1 0 2.3-5.7"/><path d="M4 4v5h5"/>',
 "rot-right": '<path d="M20 12a8 8 0 1 1-2.3-5.7"/><path d="M20 4v5h-5"/>',
 "rot-180": '<path d="M5 9a7.5 7.5 0 0 1 14 0"/><path d="M19 15a7.5 7.5 0 0 1-14 0"/><path d="M19 4v5h-5M5 20v-5h5"/>',
 "flip-h": '<path d="M12 3v18"/><path d="M9 7L4 12l5 5z"/><path d="M15 7l5 5-5 5z"/>',
 "flip-v": '<path d="M3 12h18"/><path d="M7 9l5-5 5 5z"/><path d="M7 15l5 5 5-5z"/>',
 "adjust": '<path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0"/><circle cx="16" cy="6" r="2"/><circle cx="10" cy="12" r="2"/><circle cx="18" cy="18" r="2"/>',
 "grayscale": '<circle cx="12" cy="12" r="8.5"/><path d="M12 3.5a8.5 8.5 0 0 1 0 17z" ' + F + '/>',
 "invert": frame + '<path d="M3 19L21 5v12a2 2 0 0 1-2 2z" ' + F + '/>',
 "auto-contrast": '<circle cx="12" cy="12" r="8.5"/><path d="M12 3.5v17"/><path d="M15 9.5h3M16.5 8v3M15 14.5h3"/>',
 "reset": '<path d="M3 12a9 9 0 1 0 3-6.7"/><path d="M3 4v5h5"/><circle cx="12" cy="12" r="2"/>',
 "apply": '<path d="M5 12.5l4.5 4.5L19 7"/>',
 "brightness": '<circle cx="12" cy="12" r="4"/><path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4"/>',
 # ---- layers
 "layer-new": layers.replace('M3 13l9 4.5 9-4.5','M3 13l9 4.5') + '<path d="M18 14v6M15 17h6"/>',
 "layer-up": '<path d="M12 20V5M6 11l6-6 6 6"/>',
 "layer-down": '<path d="M12 4v15M6 13l6 6 6-6"/>',
 "layer-top": '<path d="M5 4h14M12 20V9M7 13l5-5 5 5"/>',
 "layer-bottom": '<path d="M5 20h14M12 4v11M7 11l5 5 5-5"/>',
 "layer-dup": '<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3"/><path d="M14 11v6M11 14h6"/>',
 "layer-delete": '<path d="M4 7h16M10 3h4M6 7l1 13h10l1-13"/><path d="M10 11v6M14 11v6"/>',
 "add-text": '<path d="M4 6V4h12v2M10 4v15M7.5 19h5"/><path d="M18 13v7M14.5 16.5h7"/>',
 "add-image": '<rect x="3" y="5" width="14" height="12" rx="2"/><circle cx="8" cy="9.5" r="1.4"/><path d="M17 14l-4-4-7 7"/><path d="M19.5 14v7M16 17.5h7"/>',
 "flatten": layers.replace('M12 4l9 4.5-9 4.5-9-4.5z','M3 9l9 4.5 9-4.5') + '<path d="M12 2v7M9 6l3 3 3-3"/>',
 "layer-text": '<path d="M5 6h14M12 6v13"/>',
 "layer-shape": '<rect x="5" y="5" width="14" height="14" rx="1"/>',
 "layer-image": '<rect x="3" y="5" width="18" height="14" rx="2"/><circle cx="9" cy="10" r="1.5"/><path d="M21 16l-5-5-8 8"/>',
 "layer-sticker": '<path d="M12 3l2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17l-5.4 2.9 1-6.1-4.4-4.3 6.1-.9z"/>',
 # ---- tools
 "tool-select": '<path d="M5 3l14 7-6 2-2 6z"/><path d="M13 12l6 6"/>',
 "tool-pencil": '<path d="M4 20l1.2-4.2L16 5l3 3L8.2 18.8z"/><path d="M14 7l3 3"/><path d="M4 20l3.5-1"/>',
 "tool-brush": '<path d="M20 4l-9 9"/><path d="M11 13c-3-1-5 1-5 3 0 2-1 3-3 4 4 1 9 0 9-4 0-1-.4-2.3-1-3z"/><path d="M13.5 10.5l-2.5-2.5"/>',
 "tool-highlighter": '<path d="M9 15l-3 3h6l2-2"/><path d="M9 15l7-11 4 3-7 11z"/><path d="M4 21h16"/>',
 "tool-airbrush": '<path d="M14 6h4v4h-4zM12 8h2"/><path d="M18 8h3"/><circle cx="7" cy="7" r=".9" ' + F + '/><circle cx="4.5" cy="10" r=".9" ' + F + '/><circle cx="8" cy="11.5" r=".9" ' + F + '/><circle cx="5" cy="14" r=".9" ' + F + '/><path d="M16 10v8a2 2 0 0 1-2 2h-1"/>',
 "tool-calligraphy": '<path d="M12 3l5 8-5 10-5-10z"/><path d="M12 11v4"/><circle cx="12" cy="10" r="1.3"/>',
 "tool-eraser": '<path d="M3 15l9-9 7 7-6 6H8z"/><path d="M8 10l7 7"/><path d="M13 20h8"/>',
 "tool-fill": '<path d="M5 11l6-6 8 8-6 6z"/><path d="M5 11h14"/><path d="M20 15c1 1.6 1.5 2.6 1.5 3.3a1.5 1.5 0 0 1-3 0c0-.7.5-1.7 1.5-3.3z"/>',
 "tool-picker": '<path d="M14 6l4 4"/><path d="M15.5 4.5a2.1 2.1 0 0 1 3 0l1 1a2.1 2.1 0 0 1 0 3L17 11l-4-4z"/><path d="M13 9l-8 8-1 3 3-1 8-8"/>',
 "tool-text": '<path d="M5 7V5h14v2M12 5v14M9 19h6"/>',
 "tool-mosaic": '<rect x="4" y="4" width="16" height="16" rx="1"/><path d="M4 12h16M12 4v16"/><rect x="4" y="4" width="8" height="8" ' + F + ' opacity=".55"/><rect x="12" y="12" width="8" height="8" ' + F + ' opacity=".55"/>',
 "tool-crop": '<path d="M6 2v14a2 2 0 0 0 2 2h14"/><path d="M2 6h14a2 2 0 0 1 2 2v14"/>',
 "tool-hand": '<path d="M8 13V6a1.5 1.5 0 0 1 3 0v5M11 11V4.5a1.5 1.5 0 0 1 3 0V11M14 11V6a1.5 1.5 0 0 1 3 0v7"/><path d="M17 9a1.5 1.5 0 0 1 3 0v5a7 7 0 0 1-7 7h-1a6 6 0 0 1-5-2.7L4 14a1.5 1.5 0 0 1 2.4-1.8L8 14"/>',
 "tool-shapes": '<rect x="3" y="11" width="9" height="9" rx="1"/><circle cx="16.5" cy="7.5" r="4.5"/>',
 "tool-redact-box": '<rect x="3" y="7" width="18" height="10" rx="1"/><path d="M6 10.5h8M6 13.5h5"/><rect x="13" y="9" width="6" height="6" ' + F + '/>',
 "tool-redact-brush": '<path d="M20 4l-8 8"/><path d="M12 12c-3-1-5 1-5 3 0 2-1 3-3 4 4 1 9 0 9-4"/><rect x="14" y="14" width="7" height="7" ' + F + '/>',
 # ---- inspector tabs
 "tab-tools": '<path d="M12 3a9 9 0 1 0 0 18c1.2 0 1.8-.8 1.8-1.7 0-1.2-1-1.6-1-2.6 0-.9.7-1.7 1.7-1.7H17a4 4 0 0 0 4-4C21 6.5 17 3 12 3z"/><circle cx="7.5" cy="11" r="1.2" ' + F + '/><circle cx="10" cy="7" r="1.2" ' + F + '/><circle cx="15" cy="7.5" r="1.2" ' + F + '/>',
 "tab-layers": layers,
 "tab-adjust": '<path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12"/><circle cx="16" cy="6" r="2"/><circle cx="10" cy="12" r="2"/><circle cx="18" cy="18" r="2"/>',
 "tab-info": '<rect x="4" y="3" width="16" height="18" rx="2"/><path d="M8 8h8M8 12h8M8 16h5"/>',
 "tab-stickers": '<path d="M5 4h14a1 1 0 0 1 1 1v9l-6 6H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1z"/><path d="M20 14h-5a1 1 0 0 0-1 1v5"/><circle cx="9" cy="10" r=".9" ' + F + '/><circle cx="15" cy="10" r=".9" ' + F + '/>',
 "tab-redact": '<path d="M12 3l8 3v6c0 4.5-3.4 8-8 9-4.6-1-8-4.5-8-9V6z"/><rect x="8" y="10" width="8" height="4" ' + F + '/>',
 "tab-batch": '<rect x="7" y="3" width="13" height="13" rx="2"/><path d="M4 7v12a1 1 0 0 0 1 1h12"/><path d="M11 9.5h5M13.5 7v5"/>',
 "tab-settings": '<circle cx="12" cy="12" r="3"/><path d="M12 2.5l1.6 2.6 3-.6.7 3 2.7 1.4-1.3 2.8 1.3 2.8-2.7 1.4-.7 3-3-.6L12 21.5l-1.6-2.6-3 .6-.7-3-2.7-1.4L5.3 12 4 9.2l2.7-1.4.7-3 3 .6z"/>',
 # ---- misc
 "swap": '<path d="M4 8h14l-4-4M20 16H6l4 4"/>',
 "prev": '<path d="M15 5l-7 7 7 7"/>',
 "next": '<path d="M9 5l7 7-7 7"/>',
 "grip": '<g ' + F + '><circle cx="9" cy="6" r="1.6"/><circle cx="15" cy="6" r="1.6"/><circle cx="9" cy="12" r="1.6"/><circle cx="15" cy="12" r="1.6"/><circle cx="9" cy="18" r="1.6"/><circle cx="15" cy="18" r="1.6"/></g>',
 "close": '<path d="M6 6l12 12M18 6L6 18"/>',
 "dock": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18"/>',
 "float": '<rect x="3" y="4" width="18" height="16" rx="2"/><rect x="7" y="9" width="10" height="5" rx="1"/>',
 "chevron-down": '<path d="M6 9l6 6 6-6"/>',
 "check": '<path d="M5 12.5l4.5 4.5L19 7"/>',
 "fill-color": '<path d="M5 11l6-6 8 8-6 6z"/><path d="M3 22h18" stroke-width="3"/>',
 "outline-color": '<rect x="5" y="5" width="14" height="14" rx="2" stroke-width="3"/>',
 "detect": '<path d="M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3"/><path d="M8 10h8M8 14h5"/>',
 "sel-sensitive": '<path d="M12 3l8 3v6c0 4.5-3.4 8-8 9-4.6-1-8-4.5-8-9V6z"/><path d="M8.5 12l2.5 2.5 4.5-5"/>',
 "sel-all-boxes": '<rect x="3" y="4" width="8" height="6" rx="1"/><rect x="13" y="4" width="8" height="6" rx="1"/><rect x="3" y="14" width="8" height="6" rx="1"/><path d="M14 17l2 2 4-4"/>',
 "sel-no-boxes": '<rect x="3" y="4" width="8" height="6" rx="1"/><rect x="13" y="4" width="8" height="6" rx="1"/><rect x="3" y="14" width="8" height="6" rx="1"/><path d="M14 15l5 5M19 15l-5 5"/>',
 "clear-boxes": '<path d="M4 7h16M10 3h4M6 7l1 13h10l1-13"/><rect x="9.5" y="11" width="5" height="5"/>',
 "keyword": glass + '<path d="M8 9h5M8 12h3.5"/>',
 "c2pa": '<path d="M12 3l8 3v6c0 4.5-3.4 8-8 9-4.6-1-8-4.5-8-9V6z"/><path d="M14.5 10a3 3 0 1 0 0 4"/>',
 "gif": frame + '<path d="M10 9.5H8a1 1 0 0 0-1 1v3a1 1 0 0 0 1 1h2v-2.5H9M12.5 9.5v5M15.5 14.5v-5H18M15.5 12h2"/>',
 "panorama": '<path d="M2.5 6.5c6.3 2 12.7 2 19 0v11c-6.3-2-12.7-2-19 0z"/><path d="M6 14l3-3 3 3 2-2 3 3"/>',
 "keyboard": '<rect x="2.5" y="6" width="19" height="12" rx="2"/><path d="M6 10h.01M9 10h.01M12 10h.01M15 10h.01M18 10h.01M7 14h10"/>',
 "about": '<circle cx="12" cy="12" r="8.5"/><path d="M12 11v5.5"/><circle cx="12" cy="7.8" r="1" ' + F + '/>',
 "help": '<circle cx="12" cy="12" r="8.5"/><path d="M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6"/><circle cx="12" cy="17" r="1" ' + F + '/>',
 "shell": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 9l3 3-3 3M12 15h5"/>',
 "unregister": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 9l6 6M15 9l-6 6"/>',
 "batch-run": '<path d="M7 4l12 8-12 8z"/>',
 "pick-frames": frame + '<path d="M8 5v14M16 5v14"/>',
 "text-style": '<path d="M5 7V5h14v2M12 5v14M9 19h6"/><path d="M3 21h18" stroke-width="2.5"/>',
}

def main():
    os.makedirs(OUT, exist_ok=True)
    for name, body in I.items():
        with open(os.path.join(OUT, name + ".svg"), "w") as f:
            f.write(HEAD + body + "</svg>\n")
    print(f"wrote {len(I)} icons to {os.path.normpath(OUT)}")

if __name__ == "__main__":
    main()
