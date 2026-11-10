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
  below the body) adds a tail; dropping a tip on the body removes that tail — *phase A refused
  the last one; the maintainer decided after the review that it goes too (phase B): a callout
  without tails is a filled text box, the only filled text label.* Mockup 09 shows
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
- **Routing on the canvas.** *(Superseded in phase B — see "Click routing" there.)* Phase A:
  with a drawing tool, a press on a handle of the selection or on a selected object edited it as
  with Select; the Text/Callout tools edited their own objects' text; anything else drew. The tool belongs to the window (`Editor::select_tool`, pushed to the current
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

### Decided during apply — phase B (2026-10-31: properties, colour, shadow, golden matrix; tasks 3.1–3.3, 4.1)

**Maintainer decisions from the phase-A review, implemented as decided:**
- **Callouts may lose their last tail.** Dropping the only tip on the body removes it
  (`tools::callout`); the model already read, saved and drew zero tails and kept the add-tail
  handle. Spec: "zero or more tails", new scenario "No tails".
- **Click routing (drawing tools always draw).** One function decides for the press and the
  cursor: `editing::tool::route_press` → `Press::Handle` (a handle of the *current selection*:
  bend/resize what was just drawn without switching tools), `Press::EditText(id)` (Text/Callout
  tool on the *selected* text/callout of its own type), else `Press::Draw` — on empty canvas, on
  any other object, and on the body of the selected object. A polygon in progress and text
  being edited still come first (unchanged). Regression tests: `editing::tool` —
  `a_second_arrow_started_on_an_existing_arrow_is_a_new_arrow`,
  `a_second_highlight_started_inside_the_selected_one_is_drawn`,
  `the_selected_arrow_keeps_its_handles_but_not_its_line`,
  `text_tools_edit_only_the_selected_text`; the same repros on the live canvas at the end of
  `ui::canvas::tests::drawing_tools_on_a_live_canvas`. *Note:* the rule keeps the curve diamond
  of the **selected** arrow grabbable, and that diamond sits on the arrow's middle — so a second
  arrow started exactly on the middle of the arrow *just drawn* (still selected) still bends it,
  by the rule's own exception. A second arrow started on the middle of any other arrow, or
  anywhere on the selected arrow's line off its handles, draws. Spec: requirement "Drawing over
  existing objects".
- **B1 — Sequence is sequence-wide.** `ops::set_property(…, "counting", …)` (and so the sidebar)
  changes every marker of the selected markers' sequences, top-level or grouped
  (`steps::recounted`), as one `Replace`; with the step tool's defaults shown, the Sequence is
  the *current* sequence's (it shows how that sequence counts, sets the prototype for new
  sequences and recounts the current one, one undo step). Spec scenario "Sequence type for the
  whole sequence".
- **B2 — not tool defaults:** `Toolbox::INTERNAL` = `text`, `value`; `Toolbox::properties`
  leaves them out and `set_property` refuses them.
- **B3 — shadow ranges:** `value::shadow` accepts distance 0–30 px and blur 0–15 σ (the
  editor's 30 px), opacity 0–1, any finite angle (stored 0–360°); larger values are refused like
  every range-checked property. "Scale annotations" clamps scaled shadows to the same ranges
  (`common::shadow_scaled`, `Image::transform`). The ranges are mockup 18's sliders (9 px at
  30 %, 12 px at 40 %). Spec: "Shadow ranges".
- **Spec carve-out:** "Common style properties" points to a new "Style exceptions" requirement:
  highlights have no shadow, redactions neither opacity nor shadow; the sidebar rows come from
  the same rule (`properties::rows`, tested per type).

**Sidebar architecture (3.1), extending the Image/Canvas pattern of add-annotation-editor.**
- *State without widgets*: `ui::editor::properties::panel_info` computes a `PanelInfo` (type,
  mode — `Selected(n)`, `ToolDefaults`, `Editing`, `DrawingPolygon` —, the values of the first
  object or of the prototype, tail count, the step tool's next value) from the document and
  what the canvas is doing; priority: text being edited, polygon being drawn, selection (all of
  one type), tool defaults. `sidebar::State` gains `SelectionInfo::panel` and `State::Tool` (no
  selection: the selection page without Arrange).
- *Rows as data*: `properties::rows` lists a type's rows in the redline's order (`Row`), the
  colour rows with their swatch set (`colour::SwatchSet`: Full ± None, Highlight, Palette,
  Outline, Text, Shadow).
- *Widgets*: every row is declared once in `editor-window.blp` (`type_panel`); `panel::TypePanel`
  shows/reorders them per `rows` and fills them. Swatch buttons are created in code (the swatch
  sets vary, recent custom colours are dynamic) — the precedent is the per-type summary rows.
  Toggle groups, end-style tiles and swatches draw their glyphs with cairo in the widget's
  foreground colour (`glyphs`; end and line styles through the real arrow renderer), so no kit
  SVGs were added (redline "Kit additions needed" listed them as improvised); Bold/Italic/
  Underline/Strikethrough and the alignments use Adwaita's `format-text-*`/`format-justify-*`.
- *Edits*: `PanelEvent::Property` → `Canvas::set_property`: the text being edited
  (`TextEdit::set_property`, part of editing's one undo step), else the selection
  (`ops::set_property`: one `Replace` with only the objects that really change, `None` if none
  — the phantom-edit guard; undo label "Change Arrow"), else the tool's prototype. The panel
  never sends the value it shows (`TypePanel::commit`), and spin buttons ignore their own
  re-reading of a rounded text (`same_shown`) — the Arrange/Canvas phantom-edit fix applied to
  every control.
- *Previews*: sliders (an `EventControllerLegacy` sees press/release even when the range's drag
  gesture claims them), the angle dial and the colour picker send `Preview`; the canvas draws the
  restyled copies (`ops::restyled`) or the edited text with the value, without a command; the
  release (or closing the popover) commits once. Keyboard changes commit directly.
- *Step*: Sequence dropdown, shape, colour, Size (radius), Value (only with one marker
  selected), Next value (`Canvas::set_next_step`), Restart Sequence (`Canvas::restart_sequence`).

**Colour picker and eyedropper (3.2).** The popover (300 px: saturation/value area, hue and
alpha sliders, Hex/R/G/B entries, Pick From Screen, three recent custom colours) is anchored to
the colour row's swatch box (it stays while the swatches are rebuilt); dragging and typing
preview, closing or Enter commits once and remembers the colour as a recent custom colour
(`colour::remember`: newest first, no repeats, three, not palette colours), which then appears
selected before Custom (redline). Recent colours live per window, in memory (add-style-system
may persist them). The eyedropper swatch and Pick From Screen call
`capture::portal::pick_colour` — `ashpd::desktop::Color::pick().send()` then `response()`,
checked against the vendored `ashpd` 0.11.1 source (`desktop/screenshot.rs` `ColorRequest`,
`desktop/color.rs` `Color { red, green, blue }` as doubles 0–1) — through
`Editor::screen_picker`, a replaceable `Fn() -> Future<Result<Rgba, CaptureError>>`. While it
waits: the eyedropper swatch is accent-filled, the subtitle reads "Pick a colour anywhere on the
screen" and a canvas chip "Click to pick · Esc cancels" stays (redline: "If the portal does not
provide a loupe, Annota shows none; the chip stays" — Annota gets no pointer events while the
desktop picks, so it never draws a loupe and cannot show the hex value under the pointer; the
mockup's "#3584E4 ·" prefix is left out). The colour goes to whatever the properties edit when
it arrives; Cancelled does nothing; no portal / denied / other errors give a toast. Tested with a
stand-in future (live test); **the real `PickColor` call has not been run against a live
portal** (the maintainer's desktop) — to be checked by hand with
`add-screen-capture/manual-checks.md`'s other portal checks.

**Shadow editor (3.3).** An `Adw.ExpanderRow` with the enable switch (the redline's boxed row
with switch and chevron); expanded: 24 px colour swatches, the 56 px angle dial (draggable;
preview while dragging) with the Angle spin (0–359°, wraps), Distance 0–30 px, Blur 0–30 px
(shown as the CSS-style radius, stored as σ = half — the phase-A reading, kept), Opacity 0–100 %;
Reset Shadow below the card while it is expanded resets to the redline default and keeps
on/off. Shared by every type with a shadow (not highlights, not redactions).

**Golden matrix (4.1).** Ten goldens added next to phase A's twelve (`tests/render/main.rs`,
section "Property matrix"): arrow end × line style, arrow widths/end sizes/curves/lines/opacity,
pen widths/smoothing/opacity, shape kind × fill mode plus radii and dashes, text styles/
alignments/outlines (4 × 4 averages, DejaVu Sans), callout shape × fill/outline plus no/three
tails (glyphs masked), marker shape × counting (labels masked, checked as strings), redaction
kind × shape × intensity, highlight colours/opacities, shadow ranges. Each property varies
against the defaults; combinations sharing drawing code are crossed — not the full cartesian
product. Phase A's goldens regenerate byte-identical.

**Deviations from the mockups (judgement calls).**
- The selection page keeps add-annotation-editor's **Arrange** group between the type
  properties and "Select All <Type>" (mockups 02–18 end with the type rows); the tool-defaults
  state has no Arrange, as in mockup 01.
- **Opacity** is shown for callouts and step markers too (mockups 09–12 omit it; the spec's
  common style gives every object opacity except redactions).
- A callout gets a **Text colour** row (24 px Paper + palette) — mockup 09 shows dark text in
  a white callout but no control for it.
- The highlighter's **Blend** dropdown (mockup 14, "Multiply") is left out: the spec defines
  multiply only, and a dropdown with one entry would be a dead control. Its help card leaves out
  "hold Shift for a single text line height", which is not implemented (Shift squares the box).
- The **narrow layout** (mockup 20: font, size and B/I/U/S on one row in the bottom sheet)
  reuses the wide panel in the sheet.
- While a polygon is drawn the panel ends with the help card (mockup 07) — line style, opacity
  and shadow are hidden then.
- The eyedropper chip has no hex value (see above).
- Changing a selected object's style does not change the tool's defaults (they are edited
  with nothing selected, "Tool defaults").

**Verification.** Library: `ops::property_edits_are_one_step_and_only_real_changes`,
`steps::changing_the_counting_changes_the_whole_sequence`, `text_edit::sidebar_changes_while_
editing_are_part_of_the_one_step`, the routing tests above, `tool_defaults_are_prototypes` (B2),
`shadows_stay_inside_the_editor_ranges` (B3), the callout last-tail test. Bin: `properties`
(rows per type, exceptions, modes, B2, the step tool's sequence), `colour` (swatch sets,
recent colours, hex/RGB, HSV, portal colours), `glyphs`, `panel` (texts, phantom-guard
rounding, picker drawing). Live (one per process, Xvfb, `GDK_BACKEND=x11`, a private session
bus that activates nothing): `ui::editor::tests::type_properties_on_a_live_display` drives tool
defaults, a swatch (one step; again: none), spin re-read (none), end styles, a slider drag
(preview, one step, undo), the shadow editor, the colour picker, the eyedropper with a stand-in
portal (waiting state, colour used, cancel), the style exceptions, Polygon via the shape type,
B1 for a selected marker and via the tool defaults, a marker's value, Restart Sequence, and Bold
while typing (one "Add Text" step). Its screenshots (`ANNOTA_SHOTS=dir`) were compared by eye
with mockups 01, 02, 07, 08, 11, 12, 15, 16, 17, 18; the eight earlier live tests still pass.

### Track for later (from the phase-A review; not fixed in phase B)
- Step markers of a newer Annota (unknown objects) are not seen by sequence numbering, so a new
  marker can repeat their numbers.
- Ctrl+D duplicates a marker's label (the copy keeps its number and sequence).
- Moving a large blur redaction recomputes the blur every frame (~2 fps for 3.4k × 1.85k).
- Eight kit SVG sources in `docs/design/kit/icons/` are not valid XML, and the Redact icon in
  the mockups' tool strip differs from the shipped `annota-redact-symbolic` (design assets).

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.

### Open after phase A (tasks 2.1–2.8)
- ~~**Phase B:**~~ *(done, see "Decided during apply — phase B")* properties sidebar per tool (3.1) — including the shape-type switch that makes
  polygons reachable without code, Restart Sequence and Next value, the step "Value" field —
  colour picker and eyedropper (3.2), shadow editor (3.3), the golden property matrix (4.1).
- **Redaction export warning** → add-export-share task 2.7.
- **Mockup 13's transient renumber chips** ("3 → 2", 1.5 s) and the faded deleted marker are not
  built; the toast with Undo is.
- **Judgement calls to confirm** *(phase B: the last-tail and the press-on-the-selection calls
  were decided by the maintainer and changed, see phase B; the rest still stand)*: the shadow
  blur reading (σ 6 for "12 px"); the last callout
  tail cannot be removed; text keeps reading the right way round when the document is mirrored;
  new step markers take the counting of their sequence but the style of the tool defaults; a
  press on the selection with a drawing tool adjusts it instead of drawing; Ctrl+Z is ignored
  while typing; blur/pixelate do not process annotations underneath.
- **Performance:** a whole-capture blur at 4K is computed once per change (cached), but moving
  such a redaction recomputes it every frame; Pango layouts are measured per frame for text
  objects without a cache hit (cached per object by content).
- **Not verified live:** typing through a real input method (dead keys, compose) — the commit
  path is tested by calling it; pasting from a real clipboard.
