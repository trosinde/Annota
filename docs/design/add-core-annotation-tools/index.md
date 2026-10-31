# add-core-annotation-tools — mockups

Every screen uses the canonical editor shell from `docs/design/add-annotation-editor/` and shows the drawn
result on a sample capture together with the full properties sidebar. 1280×800 unless noted; light + dark,
1× and 2× in `png/`. `src/assets/` holds blur/pixelate renders of `e-relay-chat.png` (derived from kit sample content).

## Scenario → mockup

| Requirement | Scenario | Mockup | Notes |
|---|---|---|---|
| Tool switching | Key switches tool | [01-tool-switch-tooltip](png/01-tool-switch-tooltip-light@1x.png) | Hovered Arrow button, tooltip “Arrow  A”. Sidebar shows “Tool defaults” when nothing is selected. |
| Common style | Eyedropper | [16-colour-picker](png/16-colour-picker-light@1x.png), [17-eyedropper-active](png/17-eyedropper-active-light@1x.png) | Swatch row → custom popover with hex/RGB; eyedropper swatch or “Pick From Screen” → loupe. |
| Common style | (opacity, shadow) | [18-shadow-editor](png/18-shadow-editor-light@1x.png) | Shadow row expands in place. |
| Arrow | Curved arrow | [02-arrow-curve-end-styles](png/02-arrow-curve-end-styles-light@1x.png) | Diamond curve handle on the curve midpoint; end-style popover open. |
| Arrow | Double-ended | [03-arrow-double-dashed](png/03-arrow-double-dashed-light@1x.png) | Triangle/triangle dashed + circle/open + bar/square examples; Shift-constraint chip. |
| Line | Plain line | [04-line](png/04-line-light@1x.png) | Same sidebar as Arrow, both ends None. |
| Pen | Editable stroke | [05-pen](png/05-pen-light@1x.png) | Width spin focused; smoothing slider. |
| Shape | Frame | [06-shape-frame](png/06-shape-frame-light@1x.png) | Rounded rectangle, fill None, corner-radius handle. Also ellipse and filled rectangle. |
| Shape | (polygon) | [07-shape-polygon-drawing](png/07-shape-polygon-drawing-light@1x.png) | Vertices, rubber segment to cursor, closing hint, help card. |
| Text | Edit in place | [08-text-edit-in-place](png/08-text-edit-in-place-light@1x.png), [20-narrow-text-properties](png/20-narrow-text-properties-light@1x.png) (700×800) | Dashed edit frame, selection highlight, caret. |
| Callout | Two tails | [09-callout-two-tails](png/09-callout-two-tails-light@1x.png) | Two tail tips (filled handles) + add-tail handle (+) below body. |
| Callout | (shapes) | [10-callout-shapes](png/10-callout-shapes-light@1x.png) | Rectangle, ellipse, cloud. |
| Step | Auto-increment | [11-step-sequence-type](png/11-step-sequence-type-light@1x.png) | Markers 1–4, ghost preview of 5 at cursor; sequence dropdown open. |
| Step | (manual value, shapes) | [12-step-edit-value-shapes](png/12-step-edit-value-shapes-light@1x.png) | Pin marker with manual value 7; A/B rounded squares = separate sequence. |
| Step | Renumber on delete | [13-step-renumber-on-delete](png/13-step-renumber-on-delete-light@1x.png) | Ghost of deleted marker + “3 → 2”, “4 → 3” chips (transient, 1.5 s) + toast with Undo. |
| Highlighter | Text readable | [14-highlighter](png/14-highlighter-light@1x.png) | Multiply blend, 45 % yellow; text under highlight stays legible. |
| Redaction | (types) | [15-redact-blur-pixelate-solid](png/15-redact-blur-pixelate-solid-light@1x.png) | Blur (email, avatar ellipse), pixelate (selected), solid (phone, booking ref, IP). |
| Redaction | Export warning | [19-export-redaction-warning](png/19-export-redaction-warning-light@1x.png) | Alert “Make Redactions Solid?”: Cancel · Export Anyway · Convert to Solid (suggested). Matches add-export-share mockup 10 (canonical). Also on Copy, drag-out and share; never on a `.annota` save. |

## Redlines

### Defaults (new objects)
| Tool | Defaults |
|---|---|
| Arrow | Coral, 8 px, start None, end Triangle, ends 100 %, solid, opacity 100 %, shadow on |
| Line | Coral, 6 px, ends None, solid, shadow on |
| Pen | Coral, 10 px, smoothing 60, shadow on |
| Shape | Rounded rectangle, outline Coral 6 px, fill None, radius 16 px, shadow on |
| Text | Adwaita Sans 34 Bold, Coral, outline Paper 6 px, left aligned, shadow off |
| Callout | Rounded, fill Coral, text white 30 Bold, no outline, padding 18 px, 1 tail, shadow on |
| Step | Circle, Coral, 28 px radius, white 800-weight label, 3 px white ring, sequence 1,2,3, shadow on |
| Highlighter | rgb(255 222 0) at 45 %, Multiply |
| Redact | Pixelate, intensity 70, rectangle; solid colour Ink |
| Shadow | Ink, angle 45°, distance 9 px, blur 12 px, opacity 45 % (on canvas at 50 %: 3.5 px / 3 px rendered) |

### Sidebar building blocks (shared property schema)
- Heading row: type name (heading) + dim caption right (“1 selected”, “Tool defaults”, “Editing”, “Drawing polygon”, “N in document”).
- **Colour**: 7 palette swatches 32 px (Coral, Amber, Lime, Teal, Azure, Violet, Ink), second row: [None] · Paper ·
  [last custom colours] · Custom (conic) · Eyedropper (`eyedropper`, fill-1). Gap 6.
- **Width / sizes**: spin 120 px wide, unit suffix in value (“8 px”). Two-column spins 132 px with mini-label above.
- **Ends** (Arrow/Line): Start/End drop buttons (44×16 preview glyph + chevron), size spins below in %.
  Popover 252 px, 3×2 grid of 44 px tiles with captions: None, Triangle, Open, Circle, Square, Bar; selected tile = accent 25 % + 2 px accent ring.
- **Line style**: toggle group Solid / Dashed / Dotted (glyphs).
- **Shape type**: toggle group Rectangle / Rounded Rectangle / Ellipse / Polygon. Callout: Rectangle / Rounded / Ellipse / Cloud.
- **Opacity**: slider + “N %”. **Shadow**: boxed row with switch + expander chevron; expanded: Colour (24 px swatches),
  angle dial 56 px + Angle spin (°), Distance, Blur, Opacity sliders; **Reset Shadow** button.
- **Text**: font dropdown + size spin; linked Bold (Ctrl+B) / Italic (Ctrl+I) / Underline (Ctrl+U) / Strikethrough;
  linked Align Left / Centre / Align Right; Colour; Outline (24 px swatches starting with None, Paper) + Outline width.
- **Step**: Sequence dropdown (1, 2, 3 · A, B, C · a, b, c · I, II, III), Shape toggle (Circle / Square / Rounded Square / Pin),
  Colour, Size, Value (only with one marker selected, subtitle “This marker only”), Next value spin, **Restart Sequence** button.
  Restart begins a new sequence; markers keep their sequence id, deleting renumbers only the same sequence.
- **Redact**: Type toggle Blur / Pixelate / Solid, Intensity slider (disabled for Solid), Shape toggle Rectangle / Ellipse,
  Solid colour (dimmed unless Solid), warning card (warning bg 18 %) for Blur/Pixelate.
- Help cards: accent 12 % background, radius 12, `info` icon, 13 px text.
- Last item: **Select All <Type>** (Arrows, Lines, Pen Strokes, Callouts, Highlights, Redactions …).

### On-canvas handles
- Endpoints (arrow/line): 12 px white circles, 1.5 px accent border. Curve handle: 12 px accent diamond with white border on the curve at t = 0.5.
- Callout: body = 8-handle box; tail tips = filled accent circle 12 px; add-tail = 18 px accent circle with white `plus`, 23 px below body centre.
- Polygon in progress: vertices 10 px; first vertex 12 px (click closes); rubber segment 3 px dashed to cursor;
  dotted 2 px closing preview; chip “N points · Enter or double-click to finish”.
- Text editing: dashed 1 px accent frame offset 6 px; selection `--accent-a50`; 2 px accent caret; chip “Esc or click outside to finish”.
- Step placement preview: ghost marker at 60 % opacity under the crosshair, chip “Click to place N”.
- Constraint chip while drawing with Shift: “0° · Shift: 0/45/90°”.

### Colour picker popover (16)
300 px, arrow pointing to the swatch row. Saturation/value area 150 px high radius 6, hue slider 12 px, alpha slider 12 px on checker,
entries Hex (focused) / R / G / B with captions, **Pick From Screen** button (`eyedropper`), 3 recent custom colours (24 px).
Committing adds the colour as a selected swatch before the Custom swatch.

### Eyedropper (17)
Uses the xdg-desktop-portal Screenshot `PickColor` call (works on Wayland, outside the window too). While active: eyedropper
swatch filled accent, subtitle “Pick a colour anywhere on the screen”, 132 px loupe (×9, pixel grid, centre cell ring)
offset +18/+18 from the pointer, chip “#RRGGBB · Click to pick · Esc cancels”.
If the portal does not provide a loupe, Annota shows none; the chip stays.

### Redaction export warning (19)
Identical to add-export-share mockup 10 (“Pixelate warning”), which is the canonical version (maintainer decision
2026-10-31; built in `data/ui/redaction-warning.blp`). Alert 460 px: `warning` icon 32 px in `--warning`, heading
“Make Redactions Solid?”, centred body naming the count and the kind (count includes redactions inside groups):
- only pixelate: “<n> redactions use pixelate. Pixelated text can sometimes be reconstructed. Solid boxes hide it for good.”
- only blur: “<n> redactions use blur. Blurred text can sometimes be reconstructed. Solid boxes hide it for good.”
- both (drawn here, 2 blur + 4 pixelate): “<n> redactions use blur or pixelate. Blurred or pixelated text can sometimes be
  reconstructed. Solid boxes hide it for good.”
- singular for n = 1: “1 redaction uses …”.

Horizontal, equal-width buttons, 12 px apart: **Cancel** · **Export Anyway** · **Convert to Solid** (suggested). Escape =
Cancel, Enter = Convert to Solid. Convert to Solid is one undo step, then the action continues. The middle button is named
after the way out (“Copy Anyway” before Copy, etc.). Shown before export, copy, drag-out and share; never before a `.annota`
save (quick save, Save As).

### Kit additions needed (improvised as page-local glyphs)
`shape-rectangle`, `shape-rounded`, `shape-ellipse`, `shape-polygon`, `shape-cloud`, `line-solid`, `line-dashed`, `line-dotted`,
`align-left`, `align-center`, `align-right`, plus the end-style preview glyphs (none, triangle, open, circle, square, bar).
Bold/Italic/Underline/Strikethrough are drawn as letters; GNOME uses `format-text-*-symbolic` — add to the kit.
