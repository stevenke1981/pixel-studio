# Pixel Studio Visual QA

Date: 2026-07-23

Build: `dist/studio-app.exe`

Design reference: AcmeUI-Native `media-studio` + `focused-productivity` light recipe

## Evidence

- Ran the release executable as a real Windows desktop application.
- Inspected the empty state and loaded a real 1550 × 838 PNG through the executable's
  initial-path flow.
- Inspected the loaded canvas, enabled save/WebP/text/layer actions and disabled-state contrast.
- Pressed `R` and visually confirmed the rectangle tool changed to pale-blue fill, blue border and
  blue text.
- Set the native window to 1280 × 720 and read the Windows accessibility tree to confirm capture,
  all seven tools, blur, remove-C2PA, WebP and save actions remain present.
- Checked WCAG contrast ratios: body text 17.84:1, muted text 7.83:1, white-on-primary 5.38:1 and
  selected-tool text 6.34:1.

The Computer Use screenshot backend reported `SetIsBorderRequired failed (0x80004002)` on this
machine, so the documented OS screenshot fallback was used. The fallback has a known high-DPI
window-crop mismatch, so screenshot inspection and the complete accessibility tree were used
together. No black or occluded capture was accepted as evidence.

## Gate review

| Gate | Result | Notes |
| --- | --- | --- |
| Hierarchy | Pass | White panels, high-contrast borders and a light canvas clearly separate regions. |
| Primary journey | Pass | Blue capture action leads; open, paste and save stay in the command bar. |
| Tool discoverability | Pass | Full labels plus V/C/T/L/A/R/H shortcuts; selected state is unambiguous. |
| Privacy workflow | Pass | Blur, remove C2PA and WebP are separate full-width inspector buttons. |
| Empty state | Pass | One bright launch surface provides capture, open and paste. |
| Loaded state | Pass | Image preview is centered; relevant editing and export controls enable. |
| Window sizing | Pass | Major controls remain in the UI Automation tree at 1280 × 720. |
| Contrast | Pass | All measured core text/action combinations exceed WCAG AA. |
| Localization | Pass | Existing Traditional Chinese and English strings remain in use. |
| Accessibility | Pass | Windows UI Automation exposes major actions and disabled states. |

## Score

- Layout and hierarchy: 19/20
- Interaction clarity: 20/20
- Visual consistency: 19/20
- Workflow fidelity: 20/20
- Responsive/high-DPI behavior: 16/20

Total: **94/100**

The default product appearance is intentionally light-only because the requested direction is a
bright workspace. A dark appearance is not exposed in this redesign. The advanced workspace
remains dense to preserve conversion, filter, panorama, GIF and batch features, and is collapsed by
default so it does not interfere with screenshot annotation.
