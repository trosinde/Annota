# Annota feature catalogue

Benchmark: the publicly documented feature set of commercial screenshot tools (reference: Snagit 2026,
Windows + Mac). Annota re-implements *functions*, never UI, artwork or text.
Each row maps to an OpenSpec change in `openspec/changes/`. Feasibility refers to GNOME/Wayland.

Legend — Phase: **MVP** (0.1) · **P2** (0.2–0.4) · **P3** (0.5+) · ✗ not planned
Feasibility: 🟢 straightforward · 🟡 platform limits/extra work · 🔴 hard

## Capture

| Feature | Phase | Change | Feasibility | Note |
|---|---|---|---|---|
| Region capture with loupe, size readout, snapping | MVP | add-screen-capture | 🟢 | own overlay on frozen portal image |
| Window capture | MVP | add-screen-capture | 🟡 | Wayland: portal picker only |
| Full screen / multi-monitor | MVP | add-screen-capture | 🟢 | |
| Time delay with countdown | MVP | add-screen-capture | 🟢 | |
| Global hotkeys | MVP | add-screen-capture | 🟡 | GlobalShortcuts portal (GNOME ≥ 48), fallback custom keybinding |
| Include/exclude cursor | MVP | add-screen-capture | 🟢 | |
| Image from clipboard / file | MVP | add-screen-capture | 🟢 | |
| Exact capture dimensions | MVP | add-screen-capture | 🟢 | |
| Capture presets with own hotkeys | P2 | add-capture-presets | 🟢 | |
| Multiple areas in one capture | P2 | add-capture-presets | 🟢 | |
| Time-lapse capture | P2 | add-capture-presets | 🟢 | |
| Text capture (OCR from screen) | P2 | add-text-recognition | 🟢 | Tesseract |
| Scrolling capture | P3 | add-scrolling-capture | 🔴 | ScreenCast + stitching; auto-scroll needs RemoteDesktop portal |
| Step capture (click guides) | P3 | add-step-capture | 🔴 | AT-SPI click events |
| Webcam still capture | ✗ | — | | low value |
| Printer capture | ✗ | — | | Windows-specific |

## Editor & document

| Feature | Phase | Change | Feasibility |
|---|---|---|---|
| Objects stay editable, select/move/resize/rotate | MVP | add-annotation-editor | 🟢 |
| Select all of one type | MVP | add-annotation-editor | 🟢 |
| Group, z-order, snapping guides | MVP | add-annotation-editor | 🟢 |
| Undo/redo (≥ 200 steps) | MVP | add-annotation-editor | 🟢 |
| Crop, resize, rotate, flip, rounded corners | MVP | add-annotation-editor | 🟢 |
| Extend canvas, transparent canvas | MVP | add-annotation-editor | 🟢 |
| Flatten / flatten all | MVP | add-annotation-editor | 🟢 |
| Recent-captures tray, picture in picture | MVP | add-annotation-editor | 🟢 |
| Editable file format `.annota`, autosave | MVP | add-annotation-editor | 🟢 |

## Annotation tools

| Tool | Phase | Change |
|---|---|---|
| Arrow (end styles, Bézier, width, dashes) | MVP | add-core-annotation-tools |
| Line, Pen (vector, smoothing) | MVP | add-core-annotation-tools |
| Shape: rectangle, rounded, ellipse, polygon | MVP | add-core-annotation-tools |
| Text (font, outline, styles) | MVP | add-core-annotation-tools |
| Callout (shapes, multiple tails) | MVP | add-core-annotation-tools |
| Step markers (1/A/a/I, restart, manual value, renumber) | MVP | add-core-annotation-tools |
| Highlighter (multiply blend) | MVP | add-core-annotation-tools |
| Blur / pixelate / solid redaction | MVP | add-core-annotation-tools |
| Shadow, opacity, eyedropper, hex/RGB | MVP | add-core-annotation-tools |
| Magnifier with offset lens | P2 | add-advanced-annotation-tools |
| Spotlight (dim/blur outside) | P2 | add-advanced-annotation-tools |
| Stamps (original sets + custom) | P2 | add-advanced-annotation-tools |
| Cut-out with edge styles | P2 | add-advanced-annotation-tools |
| Selection with auto-fill, fill, eraser, magic wand | P2 | add-advanced-annotation-tools |
| Text-only highlight/blur | P2 | add-advanced-annotation-tools |
| Smart move, simplify | P3 | add-smart-editing |

## Styles, effects, templates

| Feature | Phase | Change |
|---|---|---|
| Quick styles, favourites | MVP | add-style-system |
| Themes (corporate palette + fonts), import/export, system-wide themes | MVP | add-style-system |
| Effects: border, edges, shadow, perspective, page curl, reflection | P2 | add-image-effects |
| Colour adjustment, filters, colour replacement, watermark | P2 | add-image-effects |
| Capture info footer | P2 | add-image-effects |
| Quick effects, auto-apply after capture | P2 | add-image-effects |
| Templates: step guide, comparison, before/after, grid, timeline | P2 | add-templates |
| Combine captures, custom templates | P2 | add-templates |

## Privacy & text

| Feature | Phase | Change |
|---|---|---|
| Grab text, edit text (local OCR) | P2 | add-text-recognition |
| Smart redaction (e-mail, phone, IP, cards, IBAN, tokens, faces) | P2 | add-smart-redaction |
| Permanent redaction in saved files | P2 | add-smart-redaction |

## Video

| Feature | Phase | Change | Feasibility |
|---|---|---|---|
| Screen/region/window recording | P2 | add-screen-recording | 🟢 ScreenCast portal |
| Mic + system audio | P2 | add-screen-recording | 🟢 |
| Webcam overlay, shapes, swap | P2 | add-screen-recording | 🟡 |
| Cursor highlight, click ripple | P2 | add-screen-recording | 🟡 |
| Draw on screen while recording | P2 | add-screen-recording | 🟡 |
| Trim, cut, combine, frame grab | P2 | add-screen-recording | 🟢 |
| MP4 / WebM / GIF export | P2 | add-screen-recording | 🟢 |

## Organise & share

| Feature | Phase | Change |
|---|---|---|
| PNG / JPG / WebP / PDF export, WYSIWYG | MVP | add-export-share |
| Quick save with filename pattern | MVP | add-export-share |
| Clipboard, drag & drop out | MVP | add-export-share |
| Share via e-mail / open with | MVP | add-export-share |
| Batch export | MVP | add-export-share |
| Library with search, tags, filters, retention | P2 | add-capture-library |
| Cloud share links, comments, AI captions | ✗ | — (privacy-first, no hosted service) |
| Office/Teams/OneDrive integrations | ✗ | — (use clipboard / drag & drop) |
