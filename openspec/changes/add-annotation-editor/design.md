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
  (`data/ui/canvas-preview.blp`). Release builds compile neither the option nor `src/ui/`. (Replaced in phase 2b-i: the option now opens the editor window.)
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

### Decided during apply — phase 2b-i (2026-10-30: editor window, sidebar, zoom OSD, autosave UI, tray)
Tasks 4.1 and 4.2 (`src/ui/editor/`, `data/ui/editor-window.blp`, `restore-dialog.blp`,
`newer-version-dialog.blp`). Crop/resize/flatten (4.3) and the `--open`/capture integration
(4.4, 4.5) are still open.

- **Reachability.** The editor window replaces the `--dev-canvas` preview: debug builds open
  `--dev-canvas FILE` in it, and the first start of a debug build offers leftover autosaves in
  it. Release builds still compile neither, and `--open` and captures still take their
  stand-in paths until 4.4/4.5. The main window is unchanged; `present_window` skips the editor
  window (style class `annota-editor`). *(Superseded in phase 2b-ii: `--dev-canvas` is gone; the
  editor is in every build, reached through `--open`, captures, imports and recovery.)*
- **One window, several documents.** Each open document is a `Session` (its own `Canvas`:
  document, history, selection, view; origin; autosave slot); the canvases sit in a `GtkStack`
  and the tray's "Open" group switches between them (add-screen-capture's mockups 12/13 open
  captures as new "tabs" there). Header bar, sidebar, OSD and tray always show the current
  session, refreshed through `Canvas::connect_changed` (new: after edits, undo/redo, selection
  and snapping changes) and `connect_view_changed` (deferred to idle: Fit changes the zoom
  inside the canvas's size allocation, where label updates were lost). The window's state is
  owned by the application (a thread-local) until the window closes — found during apply: with
  only weak references the state was dropped right after opening while the widgets lived on.
  There is no per-document close yet (no redline); closing the window closes all.
- **Header bar per D1.** New Capture (starts a capture in the launcher's default mode) · linked
  Undo/Redo (disabled when empty; tooltip "Undo Move Group" with keycaps, mockup 06) | title =
  document name, subtitle `W × H · Edited` / `· Not saved` (new capture, add-screen-capture
  mockup 12) / `· 200 %` while not fitted / `Crop · W × H` (ready for 4.3) | Show Properties
  (F9) · Copy · Export split button (arrow: Email…, Open With…, Copy to Folder…) · main menu
  (mockup 17). Copy, Export, Save and Save As belong to add-export-share and show "… arrives in
  milestone 0.1" toasts (the existing `--open` placeholder wording); the Share items are
  insensitive; dragging the title out (D1) is not built. Menu items of task 4.3 (Resize Image…,
  Rotate, Flip, Flatten All) exist with their shortcuts but are insensitive.
- **Tool strip per D2: only Select.** Later-phase tools are *hidden*, not greyed out — that
  includes Crop (task 4.3) and More Tools, whose entries all belong to later changes. `V`
  selects the tool.
- **Keys.** A capture-phase key controller on the window hands keys to the current canvas
  unless a text field or a dialog has the focus, or Space/Enter would activate the focused
  button; the canvas takes the focus when clicked. Application accelerators are used only for
  keys text fields do not use (F9, Ctrl+E/S/Shift+S, Ctrl+0/1/+/−, Ctrl+R, …): they run before
  the focused widget, so Ctrl+C (Copy, placeholder), Ctrl+Z and the editing keys stay with the
  window controller. One accelerator per action: GTK 4.22 shows no accelerator in menus for
  actions with several (found during apply); keypad and `Ctrl+=` variants are the canvas's.
- **Sidebar** (Blueprint, a `GtkStack` of document/selection pages; `sidebar::state` computes
  what to show without widgets). Document: Image (size; Resize…, Rotate & Flip and Corner
  radius show the real values but stay insensitive until 4.3), Canvas (the current background
  is ringed; the swatches stay insensitive until 4.3), Objects ("Snap to guides", "N editable
  objects", Flatten All insensitive until 4.3). Above 40 objects a per-type summary with Select
  buttons sits on top (mockup 16 draws it in place of Image/Canvas; the redline says "adds", so
  those groups stay below it). Selection: type heading with caption ("Image · 1 selected",
  "Group · 2 objects", "3 Images · Shared properties"), a group's member list and Ungroup,
  Arrange (X/Y/W/H entries apply on Enter or leaving the field, Rotation spin; one undo step
  via `ops::arrange`; several objects are placed as one box without orientation, so Rotation
  is insensitive for them), Select All <Type>. Type property panels (add-core-annotation-tools)
  go between heading and Arrange.
- **"Snap to guides" is a standalone GSettings key `snap-to-guides`** (also `Ctrl+;`). D6's
  Preferences "Editor" page does not exist yet; it can show the same key later. `show-properties`
  and `show-tray` are new keys too; the existing `window-width/-height/-maximized` keys are now
  bound by the editor window.
- **Bottom sheet below 900 px.** libadwaita 1.5 has no `AdwBottomSheet` (1.6), so the sheet is a
  `GtkRevealer` over the canvas, and an `AdwBreakpoint` (built in code) moves the properties
  between sidebar and sheet on apply/unapply; its setters hide Copy and the tray and swap in a
  main menu that starts with Copy. The canvas is not refitted above the sheet.
- **Zoom OSD and popover (mockup 07)** on the existing `render::view` math; new are only
  `view::fit_width_zoom` ("Fit Width", never above 100 % like Fit) and `view::preset` (the
  checked preset). The OSD is styled in `style.css`, not with libadwaita's `.osd`, which also
  restyled the popover. GTK draws the preset items as radio buttons, not the mockup's trailing
  check mark.
- **Autosave timer and recovery.** A 30 s `glib` timer per editor window runs
  `Session::autosave_tick` → `session::tick` (Schedule + Store from phase 1). Whether a
  document counts as unsaved depends on its origin: opened `.annota` files are their own saved
  state; images opened from files are new captures (autosaved, "Not saved"); restored autosaves
  are "Edited" and keep their slot. Closing the window removes every document's autosave (spec)
  — with no Save yet (add-export-share) that means closing discards changes without asking.
  *(Superseded in phase 2b-ii: closing keeps autosaves until Save exists.)*
  "Restore Unsaved Captures?" appears once at the primary instance's start (not for `--capture`
  or forwarded launches) over an empty editor window; all rows start checked, Restore opens the
  checked ones, Discard deletes them, unchecked ones stay for next time. Built as an `AdwDialog`
  like the existing alerts (no `AdwAlertDialog` response syntax: Ubuntu 24.04's Blueprint is
  0.10). The thumbnail is drawn in a `GtkDrawingArea` (a `GtkPicture` asks for its texture's
  size where nothing limits its height).
- **"Newer Annota Version Needed" (mockup 15)** is shown for `ArchiveError::NewerFormat` when a
  file is opened (dev option, tray); Show in Files uses `GtkFileLauncher` (OpenURI portal under
  Flatpak). The file is only read.
- **Tray data source (4.2).** "Open" = the window's documents (current one ringed, unsaved dot).
  "Recent" is meant to come from the capture library's index (add-capture-library, not built):
  until then it lists the newest 50 `.annota` and `.png` files of the save folder that are not
  open. Thumbnails come from `thumbnail.png` or the scaled image, decoded on a worker thread and
  cached by path and mtime; the listing is refreshed when the window is activated. Tiles of open
  documents are updated in place (never rebuilt during a drag). Dragging a tile onto the canvas
  (a string payload that stays inside Annota) inserts the flattened open document or the file's
  image via `ops::insert_image` (one undo step "Insert Image"); "Image inserted" offers Undo,
  which only undoes if nothing happened since. Hide Tray collapses it; with no redline for
  getting it back, an OSD-style "Show Tray" button appears bottom-left of the canvas.
  Open Library (and Ctrl+L, which bootstrap-app-shell only listed in the shortcuts window) is a
  new `app.open-library` action showing "The capture library arrives in milestone 0.1" in
  whichever window it came from.
- **Styling notes.** The empty canvas is painted in code with the canvas's own colours
  (`style-dark.css` is deprecated from libadwaita 1.6, and GTK 4.14 has no CSS media queries).
  New icons `annota-select` and `annota-library` come from the kit (strokes converted to fills).
- **Verification.** Unit tests: texts (subtitle, zoom label, headings, recovery rows, newer-version
  body, tooltips), session status and autosave ticks, sidebar state, swatches and number parsing,
  tray payloads, recent listing and thumbnails, `ops::arrange`/`insert_image`, Fit Width. One
  `#[ignore]`d live test drives the real window (open, sidebar, Select All, Arrange, zoom
  popover with its accelerators, narrow sheet and back, F9 action, snapping switch, tray
  insert/undo/redo, recent file, autosave, restore, newer-format alert, close). The live-test
  harness now fails when GTK cannot initialise and documents one test per process
  (`crate::ui::test_support`). Manually, on a private Xvfb with `dbus-run-session`, isolated
  `HOME`/cache and a memory GSettings backend, the real binary was driven with `xdotool` and
  compared with mockups 01, 06, 07, 11, 13, 14, 15, 17 in light and dark: a real drag from the
  tray, a crash (30 s autosave, `kill -9`, restart → dialog → Restore), a newer-format file sent
  to the running instance (unchanged afterwards, by checksum), F9 (xdotool sends Alt+F9 on that
  keymap; the raw key code works), Ctrl+E and Ctrl+L. In-process `WidgetPaintable` snapshots
  drop dim (opacity) text on that setup; X screenshots are reliable.

### Decided during apply — phase 2b-ii (2026-10-30: image operations, `--open`, captures in the editor)
Tasks 4.3–4.5, the close-without-saving policy and two bug fixes from the phase-2b-i review.
The model is in the library (`src/editing/{image_ops,crop,canvas_edit}.rs`,
`document::geometry::Transform`, `Bitmap::transformed`, `render::burn_in`, overlay functions)
and tested without a display; the UI is `src/ui/canvas.rs`, `src/ui/editor/{mod,sidebar,resize}.rs`,
`data/ui/editor-window.blp`, `data/ui/resize-dialog.blp`.

- **`Annotation::transform` (new defaulted hook).** Crop, rotate by 90°, flip and resize move the
  whole document, which `Arrange` cannot express (a quarter turn changes position *and*
  orientation; a resize scales positions with sizes; a mirror reverses handedness), so the trait
  gained `transform(&mut self, &Transform)`, defaulted like `handles`/`drag_handle`: a pure
  translation goes through `translate` (exact), anything else maps the object's box with
  `Transform::map_arrange` (centre exact, size scaled along the box's own axes, rotation through
  `map_rotation`). `Image` overrides it to scale border, corner radius and shadow by
  `Transform::style_scale`; `Group` transforms each member on its own (so a future arrow inside a
  group maps its end points exactly) and turns its own box. Types from add-core-annotation-tools
  with point geometry override it and map their points with `Transform::apply`.
  `Transform` = scale, then mirror (H/V), then clockwise quarter turns, then translation — only
  axis-preserving maps, so a box stays a box (exact except a rotated box under non-uniform
  scaling, approximated like `Group::set_arrange`).
- **Mirrored content stays readable.** A mirror cannot be expressed as a rotation, and mirroring
  an inserted picture or (later) text would make it unreadable; Flip moves every object to its
  mirrored place and negates its angle (the reflection of a turn by r is a turn by −r), but does
  not mirror its content. Only the base image's pixels are mirrored. *(Superseded for image
  objects in the final review fixes: they mirror their picture; the default stays.)*
- **Crop.** A mode (tool `C`, D2 places it after the other tools behind a separator), not a dialog
  (mockup 08). `editing::crop::Crop` holds the frame in whole pixels, inside the canvas, at least
  1 px; corner/edge grips and moving the frame by dragging inside it (grab distance 10 screen px);
  the aspect ratio (Free, Original = the image's ratio, 1:1, 4:3, 16:9) fits the largest frame of
  that ratio into the current one and is kept by corner grips (larger change wins, limited by the
  room from the fixed corner) and edge grips (the other side follows around the centre line).
  Applying it (`image_ops::crop`) makes the frame the canvas, keeps the image's part inside it and
  re-bases the document so the image starts at the origin again (objects translated); objects
  outside the new canvas stay, as the sidebar note says. Crop needs a frame smaller than the
  canvas that still holds image pixels (an all-extension crop would leave no image). Undo/redo,
  any image operation and switching documents end crop mode; its keys are Esc and Enter (the
  editing keys rest). The sidebar's crop page edits X/Y/W/H; the subtitle reads `Crop · W × H`.
  The frame gets a thin dark line outside its 2 px white outline for contrast on white images
  (the mockup's 1 px `rgb(0 0 6 / 40%)` box shadow).
- **Resize Image (mockup 09).** `resize::Form` links Width/Height (px or %, % limited to 1000 and to
  cairo's 32 767 px). "Scale annotations" is read from its subtitle ("Line widths and text sizes
  follow the image"): objects always keep their place on the image — positions and sizes scale
  with it — and the switch decides only whether line widths, text sizes, borders and shadows
  (`style_scale` = √(sx·sy)) scale too. The image is resampled with cairo's GOOD filter and padded
  edges; the base image's corner radius scales by the smaller factor; the canvas is scaled and
  rounded, never smaller than the image.
- **Rotate and flip** map image, canvas (including an extension) and objects through one
  `Transform`; quarter turns and mirrors copy pixels exactly (nearest filter, exact matrix), so
  Rotate Left after Rotate Right gives back the same export pixel for pixel (unit-tested).
- **Canvas edges (mockup 10).** Mockup 01 shows no grabbers at rest, so an edge's grabber appears
  when the pointer is within 10 screen px outside / 4 inside that edge (only a sliver inside, so
  objects at the canvas edge stay clickable); handles of the selection win over it. While an edge
  is dragged the view keeps its mapping (the edge follows the pointer exactly); on release a fitted
  view fits the new canvas (mockup 10's "Fit · 42 %") — every canvas-size change now refits or
  re-ranges the view (`Canvas::canvas_changed`). Limitation: at fit zoom a drag can take the edge
  past the visible area; the chip stays inside the view and the result is shown after release. An
  edge cannot enter the image (unless the canvas already ended inside it). The chip counts pixels
  beyond the image edge ("+400 px"), the dashed line marks that edge while it is positive.
- **Rounded corners.** `canvas_edit::radius_handle` at (r, r) from the image corner, offered only
  when nothing is selected and the pointer is within 9 px (so it does not cover objects); dragged
  along the diagonal, clamped to half the shorter side; the sidebar spin row does the same.
  Previews (canvas rect, corner radius) are drawn through `render::Preview`, so the document is
  untouched until release — one undo step, like object gestures.
- **Canvas background.** The swatches are live; Custom opens GTK's `GtkColorDialog` (no portal);
  the eyedropper takes the next click on the canvas and renders that one document pixel as it is
  exported (only the objects on it are drawn), made opaque; Esc cancels. Transparent pixels pick
  nothing.
- **Flatten (D3: `Ctrl+T` / `Ctrl+Shift+T`; mockups 05, 17, 18).** `render::burn_in` draws the
  image (with its rounded corners) and the objects in stacking order exactly as exported into a
  new bitmap covering the image plus the objects' visible parts on the canvas — a callout on an
  extended canvas keeps its pixels, and if that grows the image left/up, everything is re-based.
  It is transparent outside the image and the objects, so the background still shows; the corner
  radius becomes 0 (it is in the pixels). One `Batch`, no confirmation; the toast "N objects
  flattened into the image" offers Undo, which only undoes if nothing happened since. Objects of
  unknown types are kept (never destroyed). The sidebar says "No editable objects · Annotations
  are part of the image" and disables Flatten All (mockup 18).
- **Context menu (mockup 05)** — it belonged to no task, built here: a `GtkPopoverMenu` from
  Blueprint (`menu context_menu`), no arrow, at the pointer (right-click) or the selection
  (Shift+F10, Menu key). Right-clicking an unselected object selects it first. The entries are
  window actions whose sensitivity follows the selection (Group needs two objects, Ungroup a
  group). Keys the canvas already handles (Ctrl+C/D/G, Del, Ctrl+]/[) are only *shown*, through
  the menu model's `accel` attribute, and stay with the canvas — application accelerators run
  before text fields; `Ctrl+T` is a new application accelerator. GTK writes "Shift+Ctrl+…" where
  the mockup writes "Ctrl+Shift+…" (GTK's fixed modifier order, as in the main menu).
- **Main menu (mockup 17)** is wired: Resize Image…, Rotate, Flip and Flatten All act; Open… opens
  images and `.annota` documents (the file chooser filter now includes `*.annota`); New Capture,
  Preferences, Keyboard Shortcuts and About are the existing actions; Save, Save As, Export and
  Open Library stay "… arrives in milestone 0.1" toasts.
- **4.4 `--open`.** `src/ui/` is compiled in release builds now (it was debug-only), the hidden
  `--dev-canvas` option is gone, and `--open FILE` (resolved against the launching process'
  directory, also when forwarded) opens the file in the editor window; recovery is offered at the
  primary instance's start in every build. Spec: `specs/app-shell/spec.md` — the archived
  "Command-line interface" requirement cannot be MODIFIED, because a MODIFIED block must keep every
  scenario of the current one and "Open file acknowledged before the editor exists" no longer
  holds (openspec refuses to drop it, and refuses REMOVED and ADDED of the same name), so it is
  REMOVED and its whole block carried into the ADDED "Command-line options" with `--open`'s real
  scenarios. "Single application instance" is MODIFIED from add-screen-capture's version of it
  (its "`--open` keeps acknowledging with a toast" sentence would otherwise go stale) — so
  **add-screen-capture must be archived before this change**.
- **4.5 captures in the editor.** `delivery::deliver` opens "Open in Editor"/"Both" captures and
  pasted images as new documents (`editor::open_capture`): a capture is named by the file-name
  pattern (mockup 12), a pasted image "Pasted Image" (mockup 13); "Not saved"; the toast keeps
  add-screen-capture's titles, offers Copy (editor only) or nothing (Both, already copied), and
  for pasted/opened images Undo, which closes the new document and discards its autosave. Opened
  and dropped files (`import::open_file`) go to `editor::open_file`, `.annota` documents too.
  "Copy to Clipboard" is unchanged. If the editor window cannot be built, the capture is
  quick-saved as before, so it is never lost. Spec: `specs/capture/spec.md` here (where both
  changes' tasks point) MODIFIES "After-capture destination" and "Image from clipboard or file";
  "Region capture"/"Window capture" only hand the image "to the after-capture destination", which
  stays true, so they are not repeated. openspec notes that this delta can only archive once the
  `capture` spec exists — again: archive add-screen-capture first. add-screen-capture's mockups
  12/13 show a "Capture" card (Size, Source, Captured, "Saved to the library automatically") in
  the sidebar; it is not built — the source and time are not part of the document and the library
  note belongs to add-capture-library — so new captures show the document sidebar of this change.
- **Welcome page (bootstrap-app-shell mockup 01)** replaces the main window's status page: icon,
  "Capture Anything", mode cards (156 × 108, keycaps from the shortcut settings, the first one
  focused) that start a capture, and the Ctrl O / drop hint; the main window's menu gains New
  Capture, Open… and Capture Library (that change's mockup 02). The mockup's "The capture opens
  here" is written "opens in the editor": captures open in the editor window, not in this one.
- **Closing keeps autosaves (maintainer decision, temporary until add-export-share's Save).**
  Closing the editor window (Ctrl+W, close button) or quitting (`connect_shutdown`) runs one last
  autosave tick per document (`Session::close`) and never removes an autosave; only Discard in
  "Restore Unsaved Captures?" (or getting back to the saved state) does. Without this, routing
  captures into the editor (4.5) would have lost every capture closed within 30 s. To keep the
  recovery list meaningful, an image opened from a file (`Origin::Image`) is autosaved only once
  it changes (its pixels are on disk); captures and pasted images exist nowhere else and are
  autosaved from the start. The dialog no longer says "Annota closed unexpectedly" but "These
  documents have changes that are not saved in a file. Annota kept a copy of each one …" —
  true after a crash and after an ordinary close. **Follow-up:** add-export-share task 2.6 adds a
  "Save changes before closing?" confirmation with Save and reverts closing to "the autosave goes
  when the document is saved or the user discards it".
- **Bug fix: phantom Arrange edits.** The X/Y/W/H entries show 0.1 px and compared the parsed text
  with the real value, so tabbing through a fractional box made "Resize"/"Move" steps (4 bogus
  steps for X→Y→W→H). Now an entry whose text is what it showed keeps the exact value, the
  rotation spin counts as unchanged within its 0.1° display rounding, and nothing is sent if
  nothing changed (`sidebar::arrange_target`; the crop entries likewise).
- **Bug fix: open images listed under Recent.** Only `.annota` sessions kept their path, so an open
  PNG stayed in "Recent" and clicking it opened a duplicate. Every session now keeps the canonical
  path it was opened from (`Session::path`); "Recent" excludes open files of any type compared by
  canonical path, and opening a file that is open switches to it.
- **Verification.** Unit tests: transforms, bitmap transforms, every image operation (exported
  pixels compared before/after crop, rotate, flip, flatten; undo), crop frame and grips, canvas
  edges and radius gestures, overlay pixels (crop, grabber, dashed edge), labels and toasts,
  Arrange/crop entry targets (the bug), recent listing with canonical paths (the bug), resize form,
  delivery plans/toasts/names. Live (`#[ignore]`, one per process, Xvfb + GDK_BACKEND=x11): the
  editor test now also drives crop mode (C, sidebar, Enter, Esc), rotate, background swatch,
  Flatten All, the context menu, the Arrange tab-through (no phantom step), the Recent bug, a new
  capture, and closing with kept autosaves (including a change made after the last tick); a new
  canvas test drags a canvas edge, the radius handle and a crop grip and uses the eyedropper.
  Manually, the real binary (release-style `--open`, forwarded to the primary instance) was driven
  with `xdotool` on a private Xvfb inside `dbus-run-session` and compared with mockups 05, 08, 09,
  10, 17, 18 and bootstrap-app-shell 01 (dark style, the private session's default). Not verified
  live: a real capture through the portal into the editor (the portal is never invoked here) and
  the welcome cards starting one; paste from a real clipboard.

### Decided during apply — final review fixes (2026-10-30)
Items from the final reviewer pass, each decided by the maintainer before it was built.

- **B2a: boxes with turned members resize uniformly.** `Group::needs_uniform_scaling` is true
  when any member, at any depth (nested groups and their own boxes included), is turned against
  the group's box by an angle that is not a multiple of 90°. Then `Group::drag_handle` keeps the
  ratio whatever Shift says (corner: the larger change wins; edge: the other side follows around
  the centre line, as for images), and `ops::arrange` (the sidebar's W/H) makes the target
  uniform with `Group::uniform_target` (factor from the entered side, top-left kept). A
  multi-selection is a transient `Group`, so both paths cover it. `Group::set_arrange` keeps its
  per-axis approximation; it is now only reached with uniform or axis-aligned targets from
  these two paths (transforms map members on their own). Regression tests:
  `drag::multi_selection_with_a_turned_member_resizes_uniformly` (30° and 45°; before: the box
  ended at 799.3 instead of 800 px wide and the height grew on its own),
  `group::turned_members_force_uniform_scaling` (nested, round trip),
  `ops::arranging_a_group_with_a_turned_member_scales_uniformly` (before: 327 px for a typed
  ×1.5 width).
- **B2b: resize snapping uses where the handle lands.** `drag::snap_handle` first applies the
  drag unsnapped (so the type's ratio rule has run), snaps the handle's *landed* position, applies
  that, and keeps it only if the handle really ends on the line(s); otherwise it tries each axis
  alone, nearest first, moved along the line from the fixed opposite handle (which a kept ratio
  follows), and otherwise whatever the combined attempt really reached. The guides are always
  recomputed from the final handle position (`snap::lines_at`), so a guide or chip can no longer
  name a target the box does not touch. Regression tests:
  `drag::ratio_locked_resize_never_claims_a_snap_it_does_not_land_on` (before: "Canvas edge" at
  1600 for a box ending at 1640) and `drag::ratio_locked_resize_snaps_where_the_box_lands`
  (before: a corner 4 px from the edge did not snap; also two lines on different axes, the
  nearer wins).
- **Image objects mirror on Flip (maintainer decision).** `Image` gained `flip_horizontal` and
  `flip_vertical` (mirror in the object's own frame, before its rotation; drawn by mirroring the
  bitmap inside the clipped frame). `Image::transform` toggles the flag of the transform's
  mirror axis: mirroring the document maps the frame `R(r)` to `R(−r)` followed by the same
  mirror in the local frame (`M·R(r) = R(−r)·M`), so the position/angle mapping stays as it was
  and only the flag is new. Chosen over replacing the embedded PNG with a mirrored copy: exact,
  free, undo is the usual whole-object replace, flipping twice gives back the identical object,
  and `Annotation::transform` has no access to document resources. The two keys are new in
  format 1.0's `image_object` (the format has not shipped; the schema lists them as written,
  `#[serde(default)]` reads development files without them as not mirrored). The generic
  `Annotation::transform` default — used by `Group` and future text/stamp types — is unchanged.
  Tests: `image_ops::flips_mirror_image_objects_with_their_content` (the whole export is the
  old export mirrored pixel for pixel, H then V, and back; before: pixel (8, 8) was violet
  instead of amber), grouped members, rotate/resize leave the flags, files without the keys.
- **N1: objects of a newer Annota block the re-basing operations (maintainer decision).** Crop,
  rotate, flip, resize image and flatten move or re-base every object; an `UnknownObject` (also a
  group with an unknown member) cannot follow, so leaving it in place put it on the wrong image
  pixels — against "left where they are". `image_ops::has_unknown_objects` makes `rotate`,
  `flip`, `resize`, `crop`/`can_crop` and `flatten`/`flatten_all` return `None` (model-level
  refusal), and `Canvas::start_crop` refuses. The editor disables the image actions, Flatten
  Selection (moved out of the "selected" action group) and Flatten All, and makes the Crop tool
  button insensitive; their tooltips (sidebar Resize…, Rotate & Flip, Flatten All, the Crop tool)
  read "<name> · Needs a newer Annota: this document has objects from a newer version" (GTK shows
  tooltips on insensitive widgets — checked on Xvfb). Their keys (Ctrl+R, Ctrl+Shift+R, Ctrl+T,
  Ctrl+Shift+T, C) show the toast "This document has objects from a newer Annota. Update Annota
  to crop, rotate, flip, resize or flatten it." — GTK lets a disabled accelerator's key through
  to the window's key controller, verified with the real binary and `xdotool` on Xvfb. Menu
  items are just insensitive (a GMenu item has no tooltip). Extending the canvas, its background,
  corner radius and every object edit stay available. Also fixed on the way: the sidebar counted
  unknown objects as "editable objects"; it now counts selectable ones. Tests:
  `image_ops::unknown_objects_block_the_operations_that_move_every_object` (all refuse, the
  document is untouched, canvas/background/radius/nudge/duplicate still work, available again
  without the object), `sidebar::objects_of_a_newer_annota_are_not_editable`, the live
  `editor::newer_objects_block_image_operations_on_a_live_display` (actions, crop button,
  tooltips, the key toast, no crop mode, known objects still move, a plain document re-enables
  everything).

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
  *Resolved (final review fixes, B2a):* uniform scaling is forced in that case.
- **Snap guides aren't re-checked after a ratio-locked resize.** On an image corner-resize
  without Shift (ratio locked), the guide/chip can claim a snap (e.g. "Canvas edge" at x=1600)
  that the final box doesn't actually land on (box ends at x=2000) — the ratio clamp is applied
  after snapping. Low-medium severity, worth fixing before archive.
  *Resolved (final review fixes, B2b):* snapping uses the landed position and guides are
  re-checked against it.
- **The live-display GTK test harness is unreliable, not just slow.** Running both `#[ignore]`d
  GTK tests in one `cargo test ... -- --ignored` process can silently pass a test that printed
  "skipped: no display" instead of actually running, and with no display at all both pass
  without doing anything — a failed `gtk::init` doesn't fail the test. Fix before phase 2b adds
  more live tests: make a failed `gtk::init` an actual test failure, and run one GTK test per
  process (or switch to a `harness = false` integration test for these).
  *Phase 2b-i:* a failed `gtk::init` now fails the test (all three live tests, via
  `ui::test_support::init`), and one-test-per-process is documented there; nothing enforces it
  yet (a `harness = false` runner would).
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

### Open after phase 2b-i (tasks 4.1, 4.2)
- ~~Closing the editor discards unsaved changes without asking.~~ Resolved for now in phase
  2b-ii: closing keeps autosaves until Save exists; add-export-share task 2.6 brings "Save changes
  before closing?" and the final behaviour.
- **No way to close a single document** (only the whole window); no redline either. (The Undo of
  a new capture's toast closes that document; there is no general per-document close.)
- ~~The context menu belongs to no task.~~ Built in phase 2b-ii.
- **The tool strip shows Select and Crop** (D2). Arrow … Stamp arrive with
  add-core-annotation-tools; the shortcuts window (bootstrap-app-shell 07, "exactly as drawn")
  still lists them all — left unchanged. The German-keyboard Ctrl+Shift+]/[ point above is still
  open.
- ~~Image/Canvas controls are insensitive until 4.3.~~ Wired in phase 2b-ii.
- **Recent is the save folder, not the library.** add-capture-library should replace
  `tray::recent_files` with its index and keep the "Open Library" tile wiring (`app.open-library`).

### Open after phase 2b-ii (tasks 4.3–4.5)
- **Archive order:** add-screen-capture must be archived before this change (its `capture` spec
  must exist for `specs/capture/spec.md`, and `specs/app-shell/spec.md` modifies the version of
  "Single application instance" that add-screen-capture writes).
- **A new capture's sidebar** shows the document properties, not add-screen-capture mockups
  12/13's "Capture" card (size, source, capture time, "saved to the library") — needs capture
  metadata in the document and add-capture-library.
- **Canvas edge drags at fit zoom** can take the edge out of the visible area during the drag
  (the view refits on release); a live refit or auto-scroll would need a design decision.
- **Canvas extension has no keyboard path** (CLAUDE.md §7: every action without a mouse): the
  redline offers only dragging an edge and the hint "Drag a canvas edge …"; a keyboard route
  (e.g. canvas size entries) needs a design decision. Crop, resize, rotate, flip, corner radius,
  background (Custom colour) and flatten are keyboard-reachable; the eyedropper is pointer-only
  by nature.
- ~~**Image content is not mirrored by Flip**~~ — resolved in the final review fixes (maintainer
  decision): image objects mirror their picture; future text/stamp types keep the default.
- **Not verified live:** a real portal capture landing in the editor, the welcome cards starting a
  capture, pasting from a real clipboard (all driven only through their code paths in tests).
- **A 2-px sidebar artifact** appears below the selection and crop pages in the Xvfb screenshots
  (both GSK renderers); bisecting showed it is not the new crop page, and no mapped widget covers
  it — origin not found, possibly pre-existing.

