# Design: Add core annotation tools

## Mockups (design gate)

Mockups live in `docs/design/add-core-annotation-tools/` (HTML in `src/`, PNG light/dark at 1× and 2× in `png/`).
Scenario-to-mockup mapping and redlines: [docs/design/add-core-annotation-tools/index.md](../../../docs/design/add-core-annotation-tools/index.md).

Light 1× renders:

- [01-tool-switch-tooltip](../../../docs/design/add-core-annotation-tools/png/01-tool-switch-tooltip-light@1x.png)
- [02-arrow-curve-end-styles](../../../docs/design/add-core-annotation-tools/png/02-arrow-curve-end-styles-light@1x.png)
- [03-arrow-double-dashed](../../../docs/design/add-core-annotation-tools/png/03-arrow-double-dashed-light@1x.png)
- [04-line](../../../docs/design/add-core-annotation-tools/png/04-line-light@1x.png)
- [05-pen](../../../docs/design/add-core-annotation-tools/png/05-pen-light@1x.png)
- [06-shape-frame](../../../docs/design/add-core-annotation-tools/png/06-shape-frame-light@1x.png)
- [07-shape-polygon-drawing](../../../docs/design/add-core-annotation-tools/png/07-shape-polygon-drawing-light@1x.png)
- [08-text-edit-in-place](../../../docs/design/add-core-annotation-tools/png/08-text-edit-in-place-light@1x.png)
- [09-callout-two-tails](../../../docs/design/add-core-annotation-tools/png/09-callout-two-tails-light@1x.png)
- [10-callout-shapes](../../../docs/design/add-core-annotation-tools/png/10-callout-shapes-light@1x.png)
- [11-step-sequence-type](../../../docs/design/add-core-annotation-tools/png/11-step-sequence-type-light@1x.png)
- [12-step-edit-value-shapes](../../../docs/design/add-core-annotation-tools/png/12-step-edit-value-shapes-light@1x.png)
- [13-step-renumber-on-delete](../../../docs/design/add-core-annotation-tools/png/13-step-renumber-on-delete-light@1x.png)
- [14-highlighter](../../../docs/design/add-core-annotation-tools/png/14-highlighter-light@1x.png)
- [15-redact-blur-pixelate-solid](../../../docs/design/add-core-annotation-tools/png/15-redact-blur-pixelate-solid-light@1x.png)
- [16-colour-picker](../../../docs/design/add-core-annotation-tools/png/16-colour-picker-light@1x.png)
- [17-eyedropper-active](../../../docs/design/add-core-annotation-tools/png/17-eyedropper-active-light@1x.png)
- [18-shadow-editor](../../../docs/design/add-core-annotation-tools/png/18-shadow-editor-light@1x.png)
- [19-export-redaction-warning](../../../docs/design/add-core-annotation-tools/png/19-export-redaction-warning-light@1x.png)
- [20-narrow-text-properties](../../../docs/design/add-core-annotation-tools/png/20-narrow-text-properties-light@1x.png)

Design approved: 2026-10-30

## Technical decisions
- One module per tool implementing `Annotation`; shared property widgets generated from a property schema
- Golden-image tests per tool and per property
- Tool icons are symbolic SVGs from the design kit

### Decided during apply — phase A (2026-10-31: the nine tool types and drawing; tasks 2.1–2.8)
Phase A builds the object types, drawing them, their on-canvas handles and the tool strip. The
properties sidebar per tool (3.1), the colour picker with eyedropper (3.2), the shadow editor
(3.3) and the golden matrix of every property (4.1) are phase B. Every type implements
`properties`/`set_property` completely and validates ranges — the contract the sidebar builds
on — and new objects copy the tool's *prototype*, so "Tool defaults" (mockup 01's caption) is the
same `properties`/`set_property` on the prototype (`editing::tool::Toolbox`).

- **Shared style.** `Shadow::DEFAULT` follows the redline (Ink, 45°, 9 px, 45 %); its "blur
  12 px" is read as a CSS-style blur radius, i.e. a Gaussian σ of 6 — the mockups render it as
  3 px at 50 % zoom, and `Shadow::blur` stores σ (phase B's shadow editor shows whichever the
  maintainer prefers). A type's shadow is the blurred alpha of everything it draws
  (`render::shadow::drop_shadow`, like CSS `drop-shadow()`), so strokes, text outlines and
  callout tails cast one shadow; opacity applies to the object as one layer. `LineStyle` dashes
  copy the mockups' SVG (dashes 2.5 w with 1⅔ w gaps, dots every 2 w, round caps).
- **New handle kinds.** `HandleKind::End` (arrow/line ends, 12 px white circle) and
  `HandleKind::Tip` (callout tails, 12 px filled accent) — `Point` stays 10 px (polygon
  vertices). The curve diamond and the 18 px add-tail handle already existed.
- **Arrow and line** are one type (`tools::arrow::Arrow`) saved under two tags (`"arrow"`,
  `"line"`), so Select All keeps them apart. Geometry is three points: the ends and `middle`, the
  point the quadratic Bézier passes at t = 0.5 — the curve handle sits there and the line "bends
  smoothly through the handle". Dragging an end keeps the bend relative to the ends; Shift
  constrains an end (and drawing) to 0/45/90° from the other. Head sizes come from the mockups'
  SVG (6 px line → 20 × 16 triangle, 8 px → 26 × 21) and scale with the end size (25–400 %).
  The box (`arrange`) is the upright box of the curve; group scaling maps the three points;
  document transforms map them exactly.
- **Pen** strokes store the pointer's points in document pixels; width, colour, opacity and
  smoothing apply when drawn, so a new width re-renders the same path (spec "Editable stroke").
  Smoothing respaces the points (≤ 6 px) and averages neighbours (radius 0–5 points, two
  passes, ends fixed); the path runs through the midpoints with quadratic curves. Shift while
  drawing replaces everything after the last freehand point with a straight segment to the
  pointer. A click is a dot. Mockup 05 shows eight resize handles and no rotation handle.
- **Shape** is a box like an image object (moves, resizes, rotates with the generic handles);
  a polygon keeps its vertices as fractions of the box, so its resize and rotation are exactly
  the box's, and a document mirror mirrors the fractions (as image objects mirror their picture).
  Fill "None" is a transparent colour; `fill_opacity` multiplies the fill (mockup 07). A frame
  without fill is hit only on its outline, so what it frames stays clickable. A rounded
  rectangle offers its corner-radius handle at (r, r).
- **Text** layout uses Pango through `pango`/`pangocairo` 0.20 (MIT; libpango is linked through
  GTK already; `debian/copyright` refreshed) with one font context per thread whose metrics are
  *not hinted*, so a text has the same size at every zoom and in export. A text object's box is
  its text's logical extent (no wrapping); resizing scales the font (text cannot stretch, so
  every resize keeps the ratio); document transforms move its centre and keep it readable (no
  mirrored text); "Scale annotations" scales font, outline and shadow. The outline is stroked
  behind the glyphs (SVG `paint-order: stroke`, as mockup 08). "Adwaita Sans" falls back to the
  system's sans-serif font where it is missing (Ubuntu 24.04 has Cantarell).
- **Editing text in place** (`editing::text_edit::TextEdit`) works on a copy; finishing — Esc,
  a click outside, another tool, undo, an operation on the whole document, switching documents,
  closing the window — is **one** undo step: adding a new object, replacing the edited one, or deleting a
  text object whose text was emptied. A new text left empty is not added; a new callout is
  added even without text (it is a shape too). Enter types a line break; arrows, Home/End
  (Ctrl: whole text), Shift selection, Ctrl+arrows/Backspace/Delete by word, Ctrl+A, Ctrl+C/X/V
  (GTK clipboard), Ctrl+B/I/U toggle bold/italic/underline (the redline's sidebar shortcuts).
  Undo/redo wait until editing ends (Ctrl+Z is swallowed while typing). Printable keys go
  through a GTK input method attached to the canvas only while editing (dead keys, compose,
  CJK); the window's key handler passes them on and does not treat them as tool keys. Editing
  starts with a click of the Text tool (or a drag of the Callout tool), a double-click on a
  text or callout with Select (the double-click's word selected), a click with the Text or
  Callout tool on one of their objects, and Enter on a selected text or callout (keyboard path).
- **Callout** body is a box; the text wraps inside it (insets: padding, plus the inscribed
  rectangle's margin for ellipse and cloud) and the body is drawn taller when the text needs it
  ("auto-grows"; the stored height is kept, so deleting text shrinks it back). Tails are tips in
  document space: resizing the body with its own handles leaves them where they point; moving
  the callout, Arrange entries and group scaling take them along. A tail is a wedge from where
  the line from the body's centre to the tip leaves the body. Body, tails and cloud bumps are
  one shape: the fill of all parts as one layer, each part's outline clipped to outside the
  other parts, so the outline runs around the union. Dragging the add-tail handle (18 px, 23 px
  below the body) adds a tail; dropping a tip on the body removes that tail — except the last,
  which stays where it was (spec: "one or more tails"; added to the requirement). Mockup 09 shows
  no rotation handle, so callouts have none (they still turn with a group). A click with the
  callout tool makes a 320 × 96 body centred on the click; the default tail points below its left
  third. Defaults per the redline; the corner radius (18) is the mockup's.
- **Step sequences** (`editing::steps`, documented there as a state machine): each marker stores
  its sequence id, its position `number` and its counting; its label is that position in that
  counting unless a value was typed for it alone. A click joins the canvas's current sequence
  (Restart Sequence's, else the newest in the document, else 1) with the number after the
  highest of that sequence, or the "Next value" if one was set (used once). Restart Sequence
  takes an id no marker has. Deleting markers — Del, the context menu, whatever goes through
  `ops::delete` — renumbers the markers that follow them **in the same sequence only** (each
  moves down by the number of deleted markers of its sequence before it), top-level or inside
  groups, as one `Batch` with the removal, so one undo restores marker and numbers exactly.
  Flattening does not renumber (the markers stay visible as pixels). New markers count like
  their sequence (A, B → C); their style comes from the tool defaults. The toast "Step 2 deleted
  · 2 steps renumbered" with Undo is built (`canvas::Notice`); mockup 13's transient "3 → 2"
  chips and the faded deleted marker are **not** (they need a timed overlay; noted under
  "Open"). Restart Sequence and Next value are canvas methods without a button until 3.1.
- **Highlighter** draws with cairo's MULTIPLY operator at the highlight's opacity (45 % per
  the redline and mockup 14; the spec said 40 % — changed to 45 %). It has no shadow. While it
  is moved, its 35 % ghost is drawn through a group and therefore not multiplied (a cosmetic
  difference during the drag only).
- **Redaction** reads the base image's pixels through `RenderContext::document` — the design
  review's reason for giving `render` document access, and sufficient: blur and pixelate are
  computed in document pixels from `original.png` (`render::redact`), so the canvas at any zoom
  and the export match (pixelate drawn with the nearest filter, blur smooth). A blur reads 3 σ
  beyond its area, repeating the image's edge pixels; results are cached per image, area and
  effect (16 per thread), so redraws do not recompute; moving a large redaction recomputes per
  frame. Annotations underneath a redaction are not processed (they are the user's own); where
  a redaction reaches beyond the image there is nothing to hide. Intensity 100 = σ 20 px or
  32 px blocks (70: σ 14, 22 px). The tool is named Redact (D2) — the spec's "Blur `B`" now says
  Redact. The export warning moved to add-export-share task 2.7 (see tasks.md 2.8); its
  requirement is now separate in the spec ("Redaction export warning").
- **Tools and gestures** (`editing::tool`). `Tool` lists the strip's tools and their keys;
  `Draw` is a drawing tool's press–drag–release, giving one `Edit` ("Add Arrow") or a text to
  edit; `PolygonDraft` collects vertices (a click on the last vertex — a double-click's second
  press — adds none; double-click, Enter or the first vertex finish; Backspace; Esc). Drags
  below 3 screen px are clicks: arrows, lines and boxes need a drag (and a box at least 2 px
  each way), pens make a dot, steps and text place on click. The new object becomes the
  selection, so its handles are there right away (mockups 02, 03).
- **Snapping type handles** (a fix in `editing::drag`): the Select tool's generic snapping of
  type-specific handles assumed they land under the pointer. The callout's add-tail handle stays
  under the body, a Shift-constrained arrow end does not follow the pointer, a dropped tail is
  gone — snapping those moved something else onto the guide (a centred callout could not get a
  second tail). Type handles now snap only when they land under the pointer; arrow ends and tail
  tips still snap (`drag::type_handles_snap_only_where_they_land_under_the_pointer`).
- **Routing on the canvas.** With a drawing tool, a press on a handle of the selection or on a
  selected object edits it as with Select (adjust what was just drawn without switching tools);
  the Text/Callout tools edit their own objects' text; anything else draws (and clears the
  selection). The tool belongs to the window (`Editor::select_tool`, pushed to the current
  canvas on every switch); the tool defaults are one `Rc<RefCell<Toolbox>>` per window; step
  sequence state (Restart, Next value) is per document. Esc ends, in order: the eyedropper,
  text editing, a polygon, a gesture, then the drawing tool (back to Select), then the
  selection. Operations on the whole document, undo/redo, crop and switching documents end
  text editing (kept) and drop a polygon first. Drawing tools show a crosshair (Text an I-beam)
  except over what they would grab.
- **Chips** (redline): "0° · Shift: 0/45/90°" below a Shift-drawn arrow or line (the angle is
  the screen convention: counter-clockwise from the right); "N points · Enter or double-click
  to finish" by the pointer; "Esc or click outside to finish" right of the editing frame;
  "Click to place N" beside the step tool's 60 % placement preview. The header subtitle reads
  "W × H · Drawing" while a polygon is drawn (mockup 07).
- **Tool strip (D2)**: Select · Arrow, Line, Pen, Shape, Text, Callout, Step · Highlighter,
  Redact · Crop; Magnify, Spotlight, Stamp and More Tools stay hidden (later changes). The new
  buttons' tooltips show the name and the key as a keycap (mockup 01: "Arrow  A"), with
  `tooltip-text` "Arrow (A)" as the plain fallback; Select and Crop keep their text tooltips
  (Crop's carries the newer-objects reason). The icons are the kit's tool icons with strokes
  converted to fills (GTK recolours only fills; the converter is not part of the repository —
  the kit's `redact.svg` is not even valid XML, it repeats `stroke-width`). The sidebar's
  headings, "Select All <Type>" and the per-type summary now name the new types.
- **Format.** The nine types are new `$defs` of `document-1.schema.json` (format 1.0 has not
  shipped, so it stays 1.0), guarded by the schema test for every registered type; a written
  document with all of them validated with Python `jsonschema` 4.19 (draft 2020-12), and an
  unknown end style is rejected. Enums have
  no fallback variant: in a newer-minor file an unknown end style etc. keeps the whole object
  as an unknown object (the registry's existing rule), never misdrawn.
- **Verification.** Unit tests per type (defaults per the redline, rendering, hit tests,
  handles, transforms, properties round trip and validation) and per scenario: "Key switches
  tool" and "Tool stays active" (`editing::tool`, live editor and canvas tests), "Curved arrow",
  "Double-ended", "Plain line", "Editable stroke", "Frame", "Polygon", "Edit in place", "Two
  tails", "Auto-increment", "Renumber on delete" (also a restarted sequence, grouped markers,
  typed values, undo), "Text readable", "Pixelate hides detail" (also canvas = export at 200 %).
  Golden images per type (`tests/render/`); glyph areas are masked for callouts and steps and
  the text golden is compared on 4 × 4 averages with DejaVu Sans, because font rendering differs
  between systems. Two new live tests (one per process, Xvfb, `GDK_BACKEND=x11`) drive every
  tool on the canvas and the tool strip/keys in the editor; the six earlier live tests still
  pass. The real binary was driven with `xdotool` on a private Xvfb inside `dbus-run-session`
  with a bus that activates nothing, isolated `HOME`, memory GSettings and resource overlays,
  and compared with mockups 01, 02, 03, 05, 06, 08, 09, 11, 13, 14 (light).

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.

### Open after phase A (tasks 2.1–2.8)
- **Phase B:** properties sidebar per tool (3.1) — including the shape-type switch that makes
  polygons reachable without code, Restart Sequence and Next value, the step "Value" field —
  colour picker and eyedropper (3.2), shadow editor (3.3), the golden property matrix (4.1).
- **Redaction export warning** → add-export-share task 2.7.
- **Mockup 13's transient renumber chips** ("3 → 2", 1.5 s) and the faded deleted marker are not
  built; the toast with Undo is.
- **Judgement calls to confirm:** the shadow blur reading (σ 6 for "12 px"); the last callout
  tail cannot be removed; text keeps reading the right way round when the document is mirrored;
  new step markers take the counting of their sequence but the style of the tool defaults; a
  press on the selection with a drawing tool adjusts it instead of drawing; Ctrl+Z is ignored
  while typing; blur/pixelate do not process annotations underneath.
- **Performance:** a whole-capture blur at 4K is computed once per change (cached), but moving
  such a redaction recomputes it every frame; Pango layouts are measured per frame for text
  objects without a cache hit (cached per object by content).
- **Not verified live:** typing through a real input method (dead keys, compose) — the commit
  path is tested by calling it; pasting from a real clipboard.
