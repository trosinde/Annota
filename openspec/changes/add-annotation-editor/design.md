# Design: Add annotation editor canvas and document format

## Mockups (design gate)

Mockups live in `docs/design/add-annotation-editor/` (HTML in `src/`, PNG light/dark at 1× and 2× in `png/`).
Scenario-to-mockup mapping and redlines: [docs/design/add-annotation-editor/index.md](../../../docs/design/add-annotation-editor/index.md).

Light 1× renders:

- [01-editor-overview](../../../docs/design/add-annotation-editor/png/01-editor-overview-light@1x.png)
- [02-move-snap-centre](../../../docs/design/add-annotation-editor/png/02-move-snap-centre-light@1x.png)
- [03-rotate-shift-snap](../../../docs/design/add-annotation-editor/png/03-rotate-shift-snap-light@1x.png)
- [04-select-all-arrows](../../../docs/design/add-annotation-editor/png/04-select-all-arrows-light@1x.png)
- [05-context-menu](../../../docs/design/add-annotation-editor/png/05-context-menu-light@1x.png)
- [06-group-and-undo](../../../docs/design/add-annotation-editor/png/06-group-and-undo-light@1x.png)
- [07-zoom-levels](../../../docs/design/add-annotation-editor/png/07-zoom-levels-light@1x.png)
- [08-crop-mode](../../../docs/design/add-annotation-editor/png/08-crop-mode-light@1x.png)
- [09-cropped-resize-dialog](../../../docs/design/add-annotation-editor/png/09-cropped-resize-dialog-light@1x.png)
- [10-canvas-extension-transparent](../../../docs/design/add-annotation-editor/png/10-canvas-extension-transparent-light@1x.png)
- [11-tray-drag-to-canvas](../../../docs/design/add-annotation-editor/png/11-tray-drag-to-canvas-light@1x.png)
- [12-inserted-image-object](../../../docs/design/add-annotation-editor/png/12-inserted-image-object-light@1x.png)
- [13-narrow-bottom-sheet](../../../docs/design/add-annotation-editor/png/13-narrow-bottom-sheet-light@1x.png)
- [14-crash-recovery](../../../docs/design/add-annotation-editor/png/14-crash-recovery-light@1x.png)
- [15-newer-format-error](../../../docs/design/add-annotation-editor/png/15-newer-format-error-light@1x.png)
- [16-many-objects](../../../docs/design/add-annotation-editor/png/16-many-objects-light@1x.png)
- [17-main-menu](../../../docs/design/add-annotation-editor/png/17-main-menu-light@1x.png)
- [18-flatten-all-result](../../../docs/design/add-annotation-editor/png/18-flatten-all-result-light@1x.png)

Design approved: 2026-10-30

## Technical decisions
- Scene graph of `Annotation` trait objects, serde-serialised
- Command pattern for undo/redo
- Cairo renderer shared by canvas and export; canvas uses GtkDrawingArea with damage-region redraws
- Spatial index (R-tree, `rstar`) for hit-testing beyond 100 objects

### Decided during apply — phase 1 (2026-10-30: model, format, renderer, canvas)
The change is implemented in two phases. Phase 1 covers tasks 2.1–2.4 and 3.1; selection and
handles, snapping, grouping, the editor window, crop/resize/flatten and the capture/`--open`
integration (3.2–4.5) follow in phase 2.

- **Library crate.** `src/lib.rs` exposes `document`, `render` and `tools` so they are tested
  without a display and golden tests live in `tests/render/` (CLAUDE.md §3). The binary uses
  them as `annota::…`. The library has no GTK widget code.
- **Coordinates.** Document space = pixels of `original.png`, origin at its top-left. The canvas
  is an integer rectangle in that space (negative x/y after extending left/top), so extending the
  canvas never moves the image or objects; crop (phase 2) translates objects instead.
- **`Annotation` trait** (`src/document/annotation.rs`): `type_name`, `bounds`, `hit_test(point,
  tolerance)`, `render(cairo, RenderContext)`, `arrange`/`set_arrange` (generic X/Y/W/H/rotation),
  `properties`/`set_property` (keyed values shared across types), and defaulted `handles`,
  `drag_handle`, `translate`, `resources`. The UI never mutates objects in the document: it edits
  clones and submits `Command::Replace`. `drag_handle` is called on the drag-start state, so
  types compute from start + pointer. Clone/serialise/downcast plumbing comes from a blanket
  `AnnotationBase` impl. Serialisation is serde per type plus an explicit registry
  (`src/document/registry.rs`, one line per type) — no linker-section magic (`typetag`).
- **Forward compatibility** (`src/document/residual.rs`; fixed after review). When a node or
  section is read, its original JSON is diffed against what this version writes for the typed
  value; the difference (unknown keys at any depth, per-element for arrays of unchanged length)
  is kept as a *residual* and deep-merged back on write, so known keys always show the current
  state. A residual remembers its value's `"type"` and is dropped when that type changes.
  Objects of unknown types are kept verbatim (with the `images/<n>.png` entries they appear to
  use). Unknown enum values: an unknown canvas background type reads as
  `Background::Unknown` (`#[serde(other)]`), is drawn as transparent and writes nothing of its
  own, so the residual restores it; in a newer-minor file an object of a known type that does
  not parse is kept as an unknown object instead of failing the file (in a same-version file it
  is damage). Types with enums of their own should add a similar fallback variant. Types must
  not use `skip_serializing_if`.
- **Commands.** `Command` is an enum of generic operations (Insert, Remove, Replace, Reorder,
  SetCanvas, SetImage, SetMetadata, Batch). `apply` (crate-private; the UI only uses
  `History`) validates first, then returns the exact inverse. Validation keeps the document
  saveable: inserted/replacing objects need finite `bounds`/`arrange`, embedded images that
  exist (or come with the `Insert`), and JSON that reads back (catches NaN in any field). A
  failing `Batch` undoes its applied steps and drops the images it added; if undoing fails too,
  `CommandError::Rollback` reports both errors; `History` keeps inverses (200 steps), with a revision number for dirty tracking and
  autosave. Whole-object replacement instead of inverse deltas makes undo exact; bitmaps are
  shared (`Rc`), so snapshots are cheap.
- **Bitmaps.** `document::bitmap::Bitmap` wraps an immutable cairo `ImageSurface` (drawn
  directly, no per-frame copy) and caches its PNG encoding, so autosaves do not re-encode the
  original. Captures (`capture::frame::Frame`, same ARGB32 layout) convert with one copy.
  Inserted images are document resources `images/<n>.png`, referenced by id.
- **Format.** `document.json` has `format` ("1.0"), `requires` ("0.1"), `generator`, `image`,
  `canvas` (rect + background `transparent` | `color`), `metadata` (title, scale factor),
  `objects`, and `autosave` (only in autosave copies). A file with a newer *minor* version is
  written back with that minor version. PNGs are stored, the JSON deflated; writes go to a
  temporary file that is renamed. Entries are size-limited and PNG dimensions are checked
  before decoding. New default canvas background: transparent.
- **Crates** (MSRV 1.85, resolved with `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback`):
  `zip` 6 (MSRV 1.83; 7+ needs 1.88) without default features, `deflate-flate2` with
  `flate2`'s pure-Rust `rust_backend` (no zlib build dependency); `rstar` 0.13 (MSRV 1.85);
  `serde`/`serde_json`; `cairo-rs` gains the `png` feature (PNG via the system cairo, no image
  crate). All MIT/Apache/Zlib/0BSD-compatible.
- **Damage regions.** GTK 4 cannot invalidate part of a drawing area, so "damage-region
  redraws" became culling: the renderer draws only objects whose bounds touch the cairo clip,
  through the R-tree above 100 objects. This only works because the canvas widget is
  viewport-sized (see Canvas): GTK records a widget's drawing at its full allocation, so a
  content-sized drawing area made the clip the whole zoomed document (review finding: 17.5 s
  and 1.4 GB for one frame at 1600 % with 301 objects; now 0.09 s and +17 MB, release build,
  Xvfb). Shadows are blurred offscreen at device resolution,
  limited to the visible area. Caching (e.g. of shadows) is left for phase 2: profiling after
  the review fix shows shadows dominate (301 shadowed images: ~55 ms per frame fitted, ~80 ms
  at 1600 %; ~20 ms without shadows; release build on Xvfb), above the 60 fps budget.
- **Canvas.** `CanvasView` (`src/ui/canvas.rs`) is a widget implementing `GtkScrollable`
  inside a `GtkScrolledWindow`: it is allocated the visible area (no minimum size, so it
  shrinks with the window and Fit stays centred) and owns the scroll adjustments, whose range
  it sets to the zoomed content; `view::view_origin` maps document space into the view for the
  current scroll position. Fit leaves 44 px around the canvas (mockup 10: 2000 px at 42 % in
  a 928 px area) and never enlarges beyond 100 %. Bitmaps use the nearest-neighbour filter from 200 % up
  (pixel inspection), smooth below. The zoom OSD and window-level actions arrive with 4.1.
- **Preview window.** Until the editor window exists, debug builds accept a hidden
  `--dev-canvas FILE` that shows a `.annota` file or image in a bare window
  (`data/ui/canvas-preview.blp`). Release builds compile neither the option nor `src/ui/`.
- **Autosave** (`src/document/autosave.rs`): one `<id>.annota` per open document in a `0700`
  directory; `Schedule::tick(revision, dirty)` → Write / Discard / Nothing keeps idle documents
  idle; `Store::list` reports name, mtime, object count and thumbnail without decoding images.
  The 30 s timer and the "Restore Unsaved Captures?" dialog belong to phase 2.
- **Schema check.** `data/schema/document-1.schema.json` (draft 2020-12) is guarded by a unit
  test against what the code writes; full validation of a written file with Python
  `jsonschema` 4.19 passed during apply (no JSON-schema engine is a build dependency).

### Decided during apply — phase 2a (2026-10-30: selection, handles, snapping, groups, z-order)
Tasks 3.2–3.4. The logic is in the library (`src/editing/`, `src/tools/group.rs`,
`src/render/overlay.rs`) and tested without a display; `src/ui/canvas.rs` feeds it events.

- **Selection is editor state, not document content.** It is not saved and is not an undo step
  (Ctrl+Z after a click undoes the last *change*; mockup 06 names the step "Move Group"). It holds
  top-level ids in stacking order and is re-synced after undo/redo. Shift+click *adds* (the spec
  says "add"; it never removes), Ctrl+click toggles (GNOME's convention). The rubber band selects
  objects whose box (`arrange`, rotated) lies *entirely* inside it, with the same modifiers.
  Pressing an object that is part of a multi-selection keeps the selection (to drag all of it);
  releasing without moving then selects only that object. "Select all of a type" is
  `Selection::select_type` / `select_where`; its sidebar button and the "5 arrows" count chip of
  mockup 04 arrive with the sidebar (4.1).
- **Gestures commit once.** `editing::drag::Drag` decides on press: a handle of the selection
  (nearest within half its size + 3 screen px), else the topmost object (4 screen px tolerance),
  else a rubber band. While the pointer moves it keeps *preview copies* computed from the press
  state (as `drag_handle` requires); the renderer draws them in their objects' stacking places
  (`render::render_preview`), moved originals as a 35 % ghost (mockups 02, 06). Below 3 screen px
  of travel it is a click. On release the copies become one `Command::Replace` — one undo step
  per gesture, labelled from `ops::Action` + `Subject` ("Move Group", "Rotate 3 Objects"). Moves
  are rounded to whole document pixels before snapping. Escape cancels a gesture (a rubber band
  restores the selection it started from); editing keys are ignored while a drag is in progress.
- **Handles.** A single object uses its own `Annotation::handles` / `drag_handle` (so arrow end
  points etc. plug in); the box is drawn only if it has `Resize` handles. A multi-selection is
  edited as a *transient* `Group` of copies, so it resizes and rotates as one box (the box turns
  while rotating and is upright again afterwards — a multi-selection has no orientation of its
  own). Edge handles are hidden on boxes under 30 screen px so corners stay reachable. The drag
  anchor of a handle is where it is drawn (the rotation handle sits 24 screen px above the box).
- **Snapping** (`editing::snap`). Targets are collected at the press: canvas edges and centre,
  edges and centres of every other selectable object's box. Each axis snaps on its own to the
  nearest line within 6 screen px (tolerance `6 / zoom`); ties go canvas centre > canvas edge >
  object centre > object edge. Moving snaps the box's left/centre/right (top/middle/bottom);
  resize and type-specific handles snap as points on the axes they move, only on upright boxes.
  Every line the box matches after snapping is a guide; the chip names the targets. Alt suspends
  snapping (as on the capture overlay); `Ctrl+;` toggles it — in memory until the sidebar switch
  (4.1) gives it a home.
- **Groups** (`tools::group::Group`): `{rotation, children: [node…]}`; members are scene nodes
  with their own ids. The box is derived — the tight box around the members in the group's
  orientation — so it can never disagree with them; `rotation` is the only geometry stored.
  `translate` moves members exactly; `set_arrange` maps each member's centre and size from the
  old box to the new (exact for uniform scaling and for members at multiples of 90° to the group;
  otherwise the size is scaled along the member's own axes). Group/ungroup are one `Batch` each
  (remove + insert), members keep their ids, the group takes the topmost member's stacking place.
  A group with a member of an unknown type is kept whole as an unknown object (never moved without
  its unknown parts); `Annotation` gained defaulted `children()` and `renumber_nested()`
  (Duplicate gives a copied group's members new ids).
- **Ids, checked against the real `Group`.** The scene now indexes nested ids (`Scene::owner`);
  `Insert` and `Replace` refuse an object that brings an id any other object has (top-level or
  nested, or repeated inside itself), and `check_geometry` recurses into members (a group's
  bounds skip non-finite members). Phase 1's `next_id` high-water mark already covered nested ids;
  tests now cover group → save → reopen → ungroup, collisions on insert/replace and duplicate.
- **Z-order** needs nothing beyond `Command::Reorder`: `ops::restacked` computes front/back and
  one step past the next unselected object (not overlap-aware, as in most editors' "one level").
- **Duplicate** (`Ctrl+D`) puts copies on top, offset 10 × 10 document px (no redline; chosen so
  the copy is visible), and selects them.
- **Overlay** (`render::overlay`, view pixels): redline values; accent `#3584E4` (libadwaita 1.5
  has no accent-colour API), member boxes at 50 %, group box dashed 3/3, guides `#8B5CF6` reaching
  30 px beyond the canvas (mockup 02's HTML), rubber band GTK-style (accent border, 20 % fill).
- **Chips** are `GtkLabel`s with the existing `.annota-chip` / `.annota-snap-chip` classes plus
  `.annota-canvas-chip` (12 px, redline) in a `GtkOverlay` around the scrolled canvas, built in
  code like the rest of the canvas. Move: "X 580 · Y 467" for one object (mockup 02), "Δ X +560 ·
  Y +330" for a group or several (mockup 06), right of the pointer and below the moving box.
  Rotate: centred above the pointer (mockup 03); "· Shift: 15° steps" is shown while Shift is held
  (the mockup's state; the redline does not say whether it is a hint or a mode indicator). Resize:
  "W × H". Snap chip beside the top end of the vertical guide, else the left end of the horizontal.
- **Keys** are handled by the canvas until the editor window's actions exist (4.1): arrows /
  Shift+arrows, Del, Ctrl+D, Ctrl+A, Esc, Ctrl+G, Ctrl+Shift+G, Ctrl+] / Ctrl+[ (+Shift),
  Ctrl+;, Ctrl+Z / Ctrl+Shift+Z. Each nudge is its own undo step (no coalescing).
- **Not in this phase:** context menu and main-menu items (4.x), the Undo tooltip (4.1; the label
  is `History::undo_label`), double-clicking a group member to edit it alone (mockup 06's sidebar
  note; no spec requirement), Shift-constrained moves (not specified).
- **Verification.** Unit tests for selection, gestures, snapping, ops, groups and overlay pixels;
  a golden image of a rotated group; two `#[ignore]`d GTK tests
  (`GDK_BACKEND=x11 xvfb-run -a cargo test --bin annota -- --ignored`), one driving the live
  canvas through move + centre snap + chips, nudge, undo, group, group drag and Shift-rotation.
  The real `--dev-canvas` window was also driven with `xdotool` on a private Xvfb display inside
  `dbus-run-session` (screenshots compared with mockups 02, 03, 04, 06); a stale
  `/usr/local/share/annota/resources.gresource` from an earlier install had to be bypassed with
  `G_RESOURCE_OVERLAYS` for that run.

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions

### Follow-ups from the phase-2a reviewer pass — fix before archive, not blocking phase 2b
- **Non-uniform group resize is visibly wrong, not just imprecise.** Resizing a multi-selection
  or group that contains a member at a non-multiple-of-90° angle to the box (e.g. drag the E
  handle on a selection containing a 45°-rotated member) grows the box on the axis the user
  didn't drag — up to +16% measured at 30°/45° — and compounds over repeated resizes. Exact at
  0°/90°. Fix: force uniform scaling whenever any member sits at a non-multiple-of-90° angle to
  the box (cheap, exact), or explicitly scope this as a known limitation if a fix is deferred.
- **Snap guides aren't re-checked after a ratio-locked resize.** On an image corner-resize
  without Shift (ratio locked), the guide/chip can claim a snap (e.g. "Canvas edge" at x=1600)
  that the final box doesn't actually land on (box ends at x=2000) — the ratio clamp is applied
  after snapping. Low-medium severity, worth fixing before archive.
- **The live-display GTK test harness is unreliable, not just slow.** Running both `#[ignore]`d
  GTK tests in one `cargo test ... -- --ignored` process can silently pass a test that printed
  "skipped: no display" instead of actually running, and with no display at all both pass
  without doing anything — a failed `gtk::init` doesn't fail the test. Fix before phase 2b adds
  more live tests: make a failed `gtk::init` an actual test failure, and run one GTK test per
  process (or switch to a `harness = false` integration test for these).
- **SHALL-level decisions added during apply need a maintainer look**, not just an implementer
  judgement call, before this change archives: Ctrl+click-toggles-selection, the rubber band
  requiring objects "entirely inside", Duplicate's 10px offset, images keeping aspect ratio
  *unless* Shift is held (this reverses the sense implied elsewhere — check it's what's wanted),
  and z-order's "one step past the next unselected object" rule.
- **Nudge-history spam.** Every arrow-key repeat is its own undo step; holding an arrow ~7s can
  push all 200 older steps out of history. Consider coalescing consecutive nudges into one step.
- **Ctrl+Shift+]/[ (bring to front / send to back) isn't typeable on German QWERTZ** without
  AltGr for the brackets. Flag when 4.1 finalizes the real keyboard shortcuts / shortcuts window.
- **Groups with an unknown-type member hide their known content too.** A known, otherwise
  perfectly editable member becomes invisible (not drawn, not selectable, not exported) if it
  shares a group with an object of a type this Annota version doesn't understand — same
  forward-compatibility rule as phase 1, but now it can hide content the user *can* see correctly
  elsewhere in the document. Worth a note in the eventual "opened with an older/newer version"
  UI (4.1) so this isn't surprising.
