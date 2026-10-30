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

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
