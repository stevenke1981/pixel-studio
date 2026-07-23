# Pixel Studio Visual QA

Date: 2026-07-23
Build: `dist/studio-app.exe`
Design reference: AcmeUI-Native `media-studio` + `studio-neutral`

## Evidence

- Ran the release executable as a real Windows desktop application.
- Inspected the empty state with an OS-level full-screen capture.
- Loaded a real 1536 × 864 PNG through the executable's initial-path flow.
- Inspected the loaded canvas and enabled inspector actions.
- Pressed `R` and visually confirmed the rectangle tool changed to the selected violet state.
- Read the Windows accessibility tree to confirm command bar, seven tool buttons, inspector
  workflow, privacy controls and status content are exposed in order.

The Computer Use screenshot backend reported `SetIsBorderRequired failed (0x80004002)` on this
machine, so the documented OS screenshot fallback was used. No black or occluded capture was
accepted as evidence.

## Gate review

| Gate | Result | Notes |
| --- | --- | --- |
| Hierarchy | Pass | Command bar, tool rail, canvas, inspector and status are visually distinct. |
| Primary journey | Pass | Capture is the primary accent action; open and paste are immediately available. |
| Tool discoverability | Pass | Explicit labels plus V/C/T/L/A/R/H shortcuts; selected state is unambiguous. |
| Privacy workflow | Pass | Region blur, C2PA removal and WebP export remain grouped in the inspector. |
| Canvas focus | Pass | Dark neutral canvas has the largest visual area and no decorative clutter. |
| Empty state | Pass | One focused starting surface provides capture, open and paste. |
| Loaded state | Pass | Image appears centered; relevant save, WebP, text and layer controls enable. |
| Window sizing | Pass after fix | Default/minimum changed to 1180×720 / 960×640 for high-DPI displays. |
| Localization | Pass | Existing Traditional Chinese and English strings remain in use. |
| Accessibility | Pass | Windows UI Automation exposes the major controls and their disabled states. |

## Score

- Layout and hierarchy: 19/20
- Interaction clarity: 20/20
- Visual consistency: 19/20
- Workflow fidelity: 20/20
- Responsive/high-DPI behavior: 17/20

Total: **95/100**

Remaining limitation: the advanced workspace is intentionally dense because it preserves all
legacy conversion, filter, panorama, GIF and batch controls. It is collapsed by default so it does
not interfere with the screenshot annotation workflow.
