# Pixel Studio Visual QA

Date: 2026-07-23

Build: `dist/studio-app.exe`

Design reference: AcmeUI-Native `media-studio` + `focused-productivity` light recipe

## Evidence

- Ran the release executable as a real Windows desktop application.
- Inspected the empty state and loaded a real 1194 × 758 PNG through the executable's
  initial-path flow.
- Confirmed the former left rail is absent and the canvas starts at the left workspace edge.
- Confirmed all seven annotation tools, image menu and zoom menu fit in the second top row at
  1280 × 720 without wrapping to a third row.
- Inspected the loaded canvas, enabled save/WebP/text/layer actions and disabled-state contrast.
- Invoked the new top-row Clear Canvas button through Windows UI Automation and confirmed the
  status `已清除畫面，可按「恢復畫面」取回。`.
- Invoked Restore Canvas from the Project menu and confirmed the original image and
  `已恢復清除前畫面。` status returned.
- Checked WCAG contrast ratios: body text 17.84:1, muted text 7.83:1, white-on-primary 5.38:1 and
  selected-tool text 6.34:1.

The Computer Use backend reported `GetCursorPos failed: Access denied` on this machine. The
documented OS fallback was attempted, then Win32 `PrintWindow` was used when the standard helper
also encountered an invalid desktop handle. Windows UI Automation supplied the functional
interaction evidence. No black or occluded capture was accepted.

## Gate review

| Gate | Result | Notes |
| --- | --- | --- |
| Hierarchy | Pass | Two compact top rows, the canvas, inspector and status bar are clearly separated. |
| Primary journey | Pass | Capture, open, paste, save and reversible clear stay in the command bar. |
| Tool discoverability | Pass | Full labels plus V/C/T/L/A/R/H shortcuts remain visible above the canvas. |
| Canvas space | Pass | Removing the 190 px left rail materially increases preview width. |
| Privacy workflow | Pass | Blur, remove C2PA and WebP are separate full-width inspector buttons. |
| Empty state | Pass | One bright launch surface provides capture, open and paste. |
| Loaded state | Pass | Image preview is centered; relevant editing and export controls enable. |
| Window sizing | Pass | Both top rows fit at 1280 × 720 without adding a third row. |
| Contrast | Pass | All measured core text/action combinations exceed WCAG AA. |
| Localization | Pass | Existing Traditional Chinese and English strings remain in use. |
| Accessibility | Pass | Windows UI Automation exposes major actions and disabled states. |

## Score

- Layout and hierarchy: 19/20
- Interaction clarity: 20/20
- Visual consistency: 19/20
- Workflow fidelity: 20/20
- Responsive/high-DPI behavior: 18/20

Total: **96/100**

The default product appearance is intentionally light-only because the requested direction is a
bright workspace. A dark appearance is not exposed in this redesign. The advanced workspace
remains dense to preserve conversion, filter, panorama, GIF and batch features, and is collapsed by
default so it does not interfere with screenshot annotation.
