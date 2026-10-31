# Design: Add export and sharing

## Mockups (design gate)

Mockups live in `docs/design/add-export-share/` (HTML in `src/`, PNG light/dark at 1× and 2× in `png/`).
Scenario-to-mockup mapping and redlines: [docs/design/add-export-share/index.md](../../../docs/design/add-export-share/index.md).

Screens (light, 1×; every screen also has `-dark@1x/2x`):

- [01-share-menu](../../../docs/design/add-export-share/png/01-share-menu-light@1x.png)
- [02-export-dialog-png](../../../docs/design/add-export-share/png/02-export-dialog-png-light@1x.png)
- [03-export-dialog-jpg-size](../../../docs/design/add-export-share/png/03-export-dialog-jpg-size-light@1x.png)
- [04-export-dialog-webp-error](../../../docs/design/add-export-share/png/04-export-dialog-webp-error-light@1x.png)
- [05-export-dialog-pdf](../../../docs/design/add-export-share/png/05-export-dialog-pdf-light@1x.png)
- [06-quick-save-toast](../../../docs/design/add-export-share/png/06-quick-save-toast-light@1x.png)
- [07-clipboard-toast](../../../docs/design/add-export-share/png/07-clipboard-toast-light@1x.png)
- [08-drag-handle-hover](../../../docs/design/add-export-share/png/08-drag-handle-hover-light@1x.png)
- [09-drag-into-files](../../../docs/design/add-export-share/png/09-drag-into-files-light@1x.png)
- [10-pixelate-warning](../../../docs/design/add-export-share/png/10-pixelate-warning-light@1x.png)
- [11-batch-select-tray](../../../docs/design/add-export-share/png/11-batch-select-tray-light@1x.png)
- [12-batch-export-dialog](../../../docs/design/add-export-share/png/12-batch-export-dialog-light@1x.png)
- [13-batch-export-done](../../../docs/design/add-export-share/png/13-batch-export-done-light@1x.png)
- [14-email-handoff](../../../docs/design/add-export-share/png/14-email-handoff-light@1x.png)
- [15-close-confirmation](../../../docs/design/add-export-share/png/15-close-confirmation-light@1x.png)
  (task 2.6, added and approved 2026-10-31; screens 01-14 were approved 2026-10-30)

Design approved: 2026-10-30 (screens 01-14); 2026-10-31 (screen 15)

## Technical decisions
- PDF via cairo PdfSurface, vector annotations kept vector
- Clipboard via GdkClipboard texture + PNG mime
- Email via `ashpd` Email portal

### Decided during apply (2026-10-31: task 2.1)

**Module shape** (`src/export/`, library half, no GTK). `export::encode(doc, &Format, scale)`
returns the file's bytes, `export::export(doc, path, &Format, scale)` writes them with
`archive::write_atomically` (temporary file next to the target, then rename — a failed export
leaves an existing file as it was). `Format` is `Png | Jpeg { quality } | WebP { compression:
Lossless | Lossy { quality } } | Pdf(PdfOptions { vectors, page: FitToImage | A4 | Letter })`,
i.e. exactly the controls of mockups 02–05 as plain parameters; `scale` is the "Size" row (1.0 =
100 %). `DEFAULT_JPEG_QUALITY` 85 and `DEFAULT_WEBP_QUALITY` 80 are the redline defaults.
Bitmap-level `encode_png`/`encode_jpeg`/`encode_webp` are public too, for the clipboard, drag
source and batch export phases. Not called from the app yet.
- *`.annota` save already existed*: `document::archive::save` is the plain, atomic save-to-path
  that autosave's `Store::write` is built on (autosave adds only its `autosave` key).
  `export::save_annota` re-exposes it with the same error type as the exporters, nothing more —
  Save As and quick save (task 2.2) call it.
- *Errors* are one `thiserror` enum, `ExportError`: `Scale(f64)` (not finite or ≤ 0),
  `Quality(u8)` (outside 1–100), `TooLarge { format, max, width, height }` (WebP's 16383 px per
  side; JPG's 65535 cannot be reached since bitmaps stop at 32767), `Render(BitmapError)`,
  `Cairo`, `Encoder { format, reason }`, `Io`, `Archive(ArchiveError)`. Settings are checked
  before anything is drawn. `ExportError::io_kind()` gives the `std::io::ErrorKind` of an export
  or a `.annota` save alike, so the dialog phase can show mockup 04's "No permission to write to
  this folder" banner for `PermissionDenied`. Messages are English diagnostics; the dialog maps
  variants to its own gettext strings (none are user-facing yet, so `po/` is unchanged). There is
  deliberately **no "encoder unavailable" variant**: see the next point — no encoder can be
  missing at runtime, so the variant could never occur.

**Encoders: compiled in, not GdkPixbuf.** GdkPixbuf can write JPEG, but WebP only through the
separate `webp-pixbuf-loader` package (installed in this dev container, not guaranteed on Ubuntu
24.04 or Debian 13), and gdk-pixbuf 2.44 is moving its loaders to glycin. A save format that
depends on an optional package is exactly the "missing codec at runtime" risk, so:
- **JPG: `jpeg-encoder` 0.6.1** (pure Rust, `(MIT OR Apache-2.0) AND IJG`; 0.7 needs Rust 1.87,
  0.6 is the newest for MSRV 1.85). Chroma is never subsampled (4:4:4 at every quality; the
  crate's default is 4:2:0 below 90), because 4:2:0 visibly smears red arrows and coloured text,
  which is most of what Annota exports; optimised Huffman tables. JPG has no alpha: translucent
  pixels are composited onto white (exact with premultiplied pixels: `c + 255 − a`).
- **WebP: `libwebp-sys` 0.14.4** (MIT bindings) with its bundled libwebp (BSD-3-Clause, Google),
  compiled by `cc` and linked statically — one crate for lossless (`WebPEncodeLosslessRGBA`) and
  lossy (`WebPEncodeRGBA`, alpha kept lossless). Only libwebp's simple one-call API is used, in
  one small `unsafe` block (`export::raster::webp_encode`). No pure-Rust lossy WebP encoder
  exists (`image-webp` only encodes lossless). Lossy WebP is always 4:2:0 (a VP8 limit).
  Static rather than the crate's `system-dylib` feature: no new build dependency in
  `debian/control`, CI or CLAUDE.md §4, and nothing to install on the build machine; the price is
  that libwebp security fixes arrive with a crate update instead of the distro's `libwebp7`
  (Annota only encodes, its own exports, which keeps that exposure small — open question below).
- `jpeg-decoder` 0.3 is a dev-dependency only (decodes JPGs in the round-trip tests; WebP is
  decoded with libwebp itself). `cairo-rs` gains the `pdf` and `v1_16` (PDF metadata) features.
- `debian/copyright` refreshed with `build-aux/update-copyright.py`, plus two hand-written
  stanzas the script cannot know about: libwebp's BSD-3-Clause notice for
  `vendor/libwebp-sys-*/vendor/*` and the IJG licence for `vendor/jpeg-encoder-*/src/fdct.rs`,
  whose clause (2) requires "this software is based in part on the work of the Independent JPEG
  Group" in the binary's documentation (`/usr/share/doc/annota/copyright` is that).

**Raster formats are the canvas, re-encoded.** PNG/JPG/WebP encode `render::flatten(doc,
scale)` — the canvas renderer with `Options::EXPORT` — so at 100 % PNG and lossless WebP are
pixel-identical to the canvas (scenario "WYSIWYG", tested against the canvas drawn the way
`ui::canvas` draws it: screen options, offset inside a larger widget). PNG goes through
`Bitmap::png` (cairo's writer); JPG/WebP get straight RGBA with `Bitmap::pixel`'s rounding.

**PDF: the shared renderer on a `cairo::PdfSurface`, in two layers.** Calling `render::render`
on the PDF context would have been enough for "vector annotations kept vector" — but it would
also embed the whole original image with redactions merely painted on top as separate PDF
objects, and text under a solid box would stay selectable text: anyone could lift redacted
content out of the file. So the page is drawn as:
1. *One bitmap* at the export resolution, covering exactly the canvas: background, original image
   and every top-level object that contains a redaction (groups at any depth, via
   `Annotation::children`), plus — transitively — every object below a bitmap object whose
   bounds overlap it (`export::pdf::raster_positions`). This keeps whatever a redaction covers
   inside the bitmap, where only the redacted result exists, and keeps stacking order correct.
   It also drops image pixels outside a shrunken canvas.
2. *Vectors*: every other object through its own `Annotation::render`, in stacking order, so
   text is embedded font glyphs and shapes are paths.
To reuse the renderer instead of re-implementing its loop, `render` gained `render_layer(cr,
doc, viewport, options, &Layer { base, objects })`: the same clip, transformation and drawing,
optionally without background/image and with a per-position object filter (`render_inner` takes
an `Option<&Layer>`; `render`/`render_preview` pass `None`, so canvas and raster export are
unchanged — golden tests green). A test draws both layers onto pixels and gets exactly
`flatten`'s bitmap, at 100 % and 50 %. Effects that the renderer already rasterises internally
(blurred shadows) are still rasterised on the PDF page, at the page's device resolution.
- *Page geometry*: user space is output pixels; **1 px = 0.75 pt (96 px per inch)**, the CSS/GTK
  reference resolution. "Fit to image": the page is the image. A4/Letter: the page takes the
  image's orientation (landscape if wider than high), the image is centred inside a 36 pt
  (½ inch) margin and only ever shrunk to fit, never enlarged. "Keep annotations as vectors" off:
  the page is the flattened bitmap, the same pixels as the PNG.
- PDF metadata: Creator "Annota", Title = the document title if it has one. cairo writes PDF 1.7
  with compressed object streams; validated with poppler (`pdfinfo`/`pdfimages`/`pdffonts`): one
  page, one image, one embedded font subset for a text annotation.

**Tests** (`src/export/tests.rs`, 21): WYSIWYG against the canvas; PNG with translucent pixels;
scale → output size and invalid scales; JPG round trip (decoded with `jpeg-decoder`), quality
plumbing (size and error ordered by quality, 0/101/255 refused), 4:4:4 read from the frame header,
transparency on white; WebP lossless pixel-exact against the canvas (decoded with libwebp), lossy
quality plumbing + `VP8 `/`ALPH` chunks + alpha exact, the 16383 px limit; PDF header/trailer,
one page, MediaBox for fit/A4/Letter in both orientations, page layout maths, text as embedded
font vs. not with vectors off, redacted text absent from the file while text above stays vector,
the raster/vector split rules (incl. a redaction inside a group), the layered drawing equal to
`flatten`; files written atomically with no temporary files left, `PermissionDenied` from an
unwritable folder for export and `.annota` save, `.annota` save → load keeps objects, canvas,
title and pixels.

**Open questions for the maintainer** (do not block the next phases):
- libwebp static (as built) vs. the distro's shared `libwebp7` (`system-dylib`, adds
  `libwebp-dev` to Build-Depends/CI/CLAUDE.md §4): static avoids packaging changes; shared gets
  Debian's security updates.
- PDF physical size: 96 px per inch ignores the capture's `scale_factor`, so a 2× HiDPI capture
  prints twice as large as the same window captured at 1×. Dividing by `scale_factor` would print
  both at screen size.
- A4/Letter: centred, ½ inch margin, never enlarged (a small capture stays small in the middle of
  the page) — no redline covers this; top-aligned or fill-width are the alternatives.

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.

### Decided during apply (2026-10-31: task 2.6 mockup, and maintainer follow-ups on task 2.1)
- **Mockup 15 approved as drawn** (Cancel/Discard/Save, Save as default response, same Discard
  treatment as the add-annotation-editor recovery dialog). See
  `docs/design/add-export-share/index.md` "Save changes before closing (15)" for the redlines.
- **Several unsaved documents in one window:** one alert per unsaved document, in the tray's
  Open order, switching to each document first so it is visible behind its alert; Cancel at any
  point stops the whole close/quit. Confirmed over the alternative (one alert listing every
  unsaved document with checkboxes, GNOME Text Editor style) — no second mockup needed.
- **Screens 01-14 redrawn with the editor's real header** (task 1.3, done 2026-10-31): Export is
  the built `Adw.SplitButton` (Export + a dropdown arrow opening Share: Email…/Open With…/Copy
  to Folder…, per `data/ui/editor-window.blp`), not a plain button — 01's Share popover now
  opens from that arrow, matching the built menu's contents (dropped "Copy Image"/"Save As…",
  which aren't in the real menu). 08/09's drag affordance moved to the window title area (no
  other drag-out control is built yet; task 2.3 will build the real one, per D1 "dragging the
  title/subtitle drags the flattened image out").
- **Other staleness in 01-14 left alone for now** (maintainer decision, 2026-10-31): tool strip
  order predates D2, and the tray/sidebar still show an older layout (e.g. screen 11 assumes a
  tray "Select" button the built tray doesn't have). Fix each screen when its own task is
  actually implemented (11/12/13 when task 2.5's batch-selection entry point is decided,
  tool-strip/tray/sidebar screens as whichever task touches them), not as a separate pass now.
- **add-annotation-editor mockup 01 and add-core-annotation-tools mockup 19** also draw Export
  as a plain button without the dropdown arrow; maintainer wants those fixed too for
  consistency, even though those changes are otherwise done and awaiting live-portal testing —
  tracked as a small follow-up, not touching anything else in either change.
- **PDF physical size now divides by the capture's device scale** (`Metadata::scale_factor`):
  a 2× HiDPI capture kept at device pixels used to print twice as large as the same window
  captured at 1×; `page_layout` in `src/export/pdf.rs` now takes `doc_scale` and divides
  `POINTS_PER_PIXEL` by it, so the physical page size depends only on the window's logical
  size, not which display captured it. Fixed directly (small, self-contained change to task
  2.1's already-landed code) with a regression test,
  `paper_layout_divides_by_the_captures_device_scale`, in `src/export/tests.rs`.
- **PDF page layout stays centred, shrink-to-fit-only** on A4/Letter — no change from what task
  2.1 already built.
- **WebP stays statically linked** (`libwebp-sys`, bundled libwebp) rather than switching to
  Debian/Ubuntu's system `libwebp` — no change from what task 2.1 already built; revisit only if
  a future CVE in libwebp makes "needs an Annota rebuild to pick up a fix" matter in practice.

### Decided during apply (2026-10-31: export dialog, task 2.2 quick save + Save As, task 2.7 in part)

**Export dialog** (`src/ui/editor/export_dialog.rs`, `data/ui/export-dialog.blp`; tasks.md
2.1a). `Ctrl+E` and the header's Export replace the "arrives in milestone 0.1" placeholder.
- *Built per redlines 02–05:* AdwDialog 560 px, Cancel | "Export Image" | Export (suggested,
  default widget; Enter in the name exports); preview row (160 × 100 thumbnail drawn once,
  radius 6, 1 px border; the document's name; "W × H px · FORMAT · ≈ SIZE"; "Pixel-identical to
  the canvas at 100 %", shown for every format as drawn); list 1 File name (extension dim,
  not editable) + Folder (folder button: GTK's folder chooser); list 2 Format toggle (the
  existing `.annota-segmented` style, libadwaita 1.5 has no `AdwToggleGroup`), Compression
  (WebP), Quality 1–100 value right (JPG; lossy WebP), Size menu, Flatten annotations (on,
  insensitive, raster only), Keep annotations as vectors + Page size (PDF only).
- *The values live in a widget-free `Form`* (format, qualities, PDF options, size, name, folder
  → `export::Format`, scale, output size, path, problems); the widgets only show it. Every new
  dialog starts from the preferences (`default-export-format`, `export-folder`, the redline's
  quality defaults) — nothing is remembered between exports. **WebP starts Lossless** (no redline
  says; mockup 04 shows Lossy as a user choice): it keeps "pixel-identical" true by default.
- *Size*: a flat MenuButton labelled "100 % · 1600 × 1000 px"; its menu has the four presets
  with pixel sizes (radio state) and "Custom Width…", an AdwAlertDialog with a width spin (1 to
  32767 px; the height follows the aspect ratio; the button then shows the rounded percentage).
- *Estimated size*: the real encoder, 250 ms after the settings last changed, on the main loop.
  Outputs up to 0.5 MP are encoded whole (exact); larger ones at a smaller scale with ~0.5 MP
  and scaled by the pixel count — "≈" by design, cheap enough for a debug build. Shown in
  decimal KB/MB like GNOME Files, the decimal separator translatable.
- *Banner (mockup 04)*: checked live on every change, before anything is written — the folder
  (or, if it does not exist yet, the nearest existing folder above it) without
  `access::can-write` (GIO) → "No permission to write to this folder" + Choose Folder…; a file
  of that name → "A file with this name already exists" + **Replace** (red label), which exports
  over it; an output larger than the format allows (WebP 16383 px, otherwise the renderer's
  32767 px) → its own text. Export stays disabled while one of those shows. Failures only the
  write can find (disk full, I/O) show in the same banner afterwards, without a button, and
  Export stays available to retry. `ExportError` → banner text is `saving::problem`
  (`io_kind()` for file system errors), shared with quick save. The folder is created if missing.
- *After exporting* the dialog closes and a toast "Exported as “Checkout flow.png”" with Show in
  Files appears — **not drawn** in the mockups (only batch export has a toast, mockup 13); worded
  like mockup 06's save toast. Exporting is not saving: the document's status and autosave do
  not change. (add-annotation-editor's capture delta says a new capture is autosaved "until
  saved or exported" — that wording predates this decision; open question below.)
- *Deviation from the mockups*: the extension label sits at the end of the File name row (an
  `AdwEntryRow` suffix), not directly after the typed name as drawn — libadwaita has no inline
  non-editable tail for an entry; a typed extension of the chosen format is ignored ("Shot.png"
  stays "Shot.png", not "Shot.png.png").
- The PDF's metadata title is the document's own title (see quick save below for why that can
  differ from the window title after a save).

**Quick save and Save As** (task 2.2; spec export-share "Quick save", "Saved document").
- `Ctrl+S` writes the editable `.annota` (it is a save, not an export): over the document's own
  `.annota` file if it has one (opened from or saved to it, or restored from an autosave that
  belongs to one); otherwise — new capture, pasted image, image opened from a PNG/JPG — into
  the `save-folder` preference (created if missing), named after `filename-pattern` (glib
  strftime; `/` becomes `-`; empty → "Annota"), with " 2", " 3" … added rather than replacing
  another file. No dialog. `Ctrl+Shift+S` opens GTK's file chooser (`gtk::FileDialog`; GTK uses
  the FileChooser portal only where it decides to, e.g. sandboxed — never exercised in tests),
  in the file's folder or the save folder, `.annota` appended if missing.
- *A saved document* belongs to its file: `Session`'s origin/source/path/name became `Cell`/
  `RefCell` so `Session::saved_as` can switch them; the window title and tray tile take the
  file's name, the subtitle reads "~/Pictures/Annota · Saved" while nothing changed since
  (mockup 06), and the history is marked saved. **The autosave is removed right away**
  (`Session::forget_autosave`; the spec allows "at the next tick").
- *The title in the file*: `export::save_annota_named` writes the document with its title set to
  the file's name (a copy; the open document's title is not changed, because a title change is
  not an undoable edit and must not be one either). Reopening the file shows the same name as
  the window did. Side effect: until the next save, the open document's own title (used by its
  autosave and a PDF export) is still the old one.
- *Failure* (no permission, disk full …): a toast with the same words as the export dialog's
  banner and **Save As…**, high priority; the document stays open and unsaved and keeps its
  autosave. No mockup draws this; the redline only says "shown as for Ctrl+S".
- `Ctrl+E`, `Ctrl+S` and `Ctrl+Shift+S` do nothing while a dialog is open over the window
  (window accelerators still fire then; otherwise a second export dialog, or a save behind the
  redaction warning).
- Closing still keeps autosaves (add-annotation-editor 2b-ii) until task 2.6; a document saved
  and unchanged has none to keep.

**Redaction warning** (task 2.7, in part; spec annotation-tools "Redaction export warning",
`specs/annotation-tools/spec.md`).
- *One entry point*, `Editor::guard_redactions(purpose, proceed)`: counts the current document's
  blur and pixelate redactions (`annota::editing::redactions::reversible`, at any depth through
  `tools::redaction::redactions_in`, the walker `export::pdf::contains_redaction` now uses too);
  none → `proceed` at once; otherwise the alert, then Cancel → nothing, the middle button →
  `proceed`, Convert to Solid → `canvas.run(redactions::make_solid)` then `proceed`.
- *Conversion*: one `Command::Replace` of every top-level object that holds a blur/pixelate
  redaction — a redaction is copied with `set_property("redaction", Choice("solid"))`, a group is
  copied with that done to its members at any depth (downcast to `Group`, members by
  `children`) — executed as one undo step, `Action::MakeSolid`, "Undo Convert Redactions to
  Solid". Geometry, ids and colour stay; solid boxes use the colour each redaction already has.
- *The alert follows mockup 10* (`data/ui/redaction-warning.blp`: AdwDialog 460 px, 32 px
  `dialog-warning-symbolic` in the warning colour, "Make Redactions Solid?", body, Cancel ·
  Export Anyway · **Convert to Solid**, Enter converts, Escape cancels), as the maintainer's
  brief for this phase says — not tasks.md's paraphrase nor add-core-annotation-tools mockup 19
  ("Redactions May Be Reversible", stacked "Convert to Solid and Export" / "Export As Is" /
  Cancel). The body names the kind: "use pixelate" / "use blur" as drawn, and — not drawn — "use
  blur or pixelate. Blurred or pixelated text …" when there are both.
- *Middle button*: "Export Anyway" before an export (as drawn). **Superseded below** (2026-10-31
  maintainer follow-up): a save was decided to never show this warning at all — `.annota` keeps
  the original image under every redaction regardless, so no "Save Anyway" exists any more;
  `Purpose::Save` was removed rather than kept unused.
- *When*: before the export dialog opens (mockup 10 shows the alert over the editor, not over the
  dialog; "then the export continues" = the dialog opens, already showing the solid result).
- *Covered call sites (as of this entry, now superseded — see the 2026-10-31 "Decided during
  apply" entry below for the current state)*: Export (`Ctrl+E`, header Export), quick save
  (`Ctrl+S`), Save As (`Ctrl+Shift+S`). *Not yet*: Copy (`Ctrl+C`) and drag-out (task 2.3), Share
  (2.4), batch export (2.5) — they are to call `guard_redactions` too (batch: per document, or
  once for the selection — to decide with 2.5). Task 2.7 stays unticked until then.

**Spec deltas.** `specs/annotation-tools/spec.md` MODIFIES "Redaction export warning" — copied
from add-core-annotation-tools' delta (that change is not archived, so there is no
`openspec/specs/annotation-tools/` yet) and rewritten as the real requirement with five
scenarios. **Archive order: add-core-annotation-tools before add-export-share** (`openspec
validate` notes that archiving would refuse a MODIFIED delta without the base spec, the same
situation as add-annotation-editor's deltas on add-screen-capture). `specs/export-share/spec.md`:
"Export formats" split into "Export formats" + "Export dialog" (banners), "Quick save" made
concrete (`.annota`, own file vs. pattern, never replacing) plus "Saved document" (title,
autosave, toast, failure) — strict validation refuses requirement texts over 500 characters.
`proposal.md` lists `annotation-tools` under Modified Capabilities.

**Tests.** Unit (no display): `editing::redactions` (counting at any depth, one undo step that
converts all and one Undo that restores all, ids and geometry kept, no phantom step);
`export::tests::saved_files_are_titled_after_their_name`; `ui::editor::saving` (error → banner
text/button/blocking, pattern names and numbering, own file, `/` and empty names, `.annota`
appended, toast texts); `ui::editor::export_dialog` (redline defaults, controls → `Format`/scale/
size, preset and custom-size labels, typed extensions, problems before writing incl. Replace and
size limits, the GIO folder check against a read-only temporary folder, summary/size texts, the
estimate equal to the real encoder for small images and available for all formats);
`ui::editor::redaction_warning` (body variants, middle button); `ui::labels` (undo label).
Live (`#[ignore]`, `GDK_BACKEND=x11 xvfb-run -a dbus-run-session cargo test --bin annota --
--ignored --exact ui::editor::tests::export_and_save_on_a_live_display`, private GSettings
backend and temporary folders only, no file chooser or portal opened): Ctrl+E → warning →
Cancel; Ctrl+E again → warning → Convert (undo label, group member converted) → Export Image
opens (cancelled, nothing written yet); Ctrl+S → no warning (save never shows it; nothing
reversible left anyway) → one pattern-named `.annota` in the save folder with solid redactions
and the file's title, header title/subtitle, toast, autosave gone; edit + Ctrl+S → saved over
the same file, still no warning; export dialog PNG → JPG (rows, size label, estimate) → Export
→ JPG file, dialog closed; Ctrl+E/Ctrl+S ignored while it is open; existing file → banner,
Export disabled, Replace; read-only folder → banner, Choose Folder…, Export disabled; failed
quick save → toast with Save As…, "Not saved", autosave kept. A live test because this wiring
(session, history, autosave, toasts, dialogs) is not a pure function; such tests have
precedent here (eleven in `ui::editor` and `ui::canvas`), and the pure parts are unit-tested
regardless.
- Seen while running every live test: `quick_styles_on_a_live_display` fails in this dev
  container on the destructive menu item's red text with or without this change's CSS (checked
  against the previous `style.css`) — an environment/theme issue to look at separately, not a
  regression here.

### Decided during apply (2026-10-31: maintainer follow-ups on the export dialog/save/warning phase)
- **The redaction warning does not apply to saves, dropped from Ctrl+S/Ctrl+Shift+S.** An
  `.annota` file contains `original.png` regardless of what a redaction is converted to, so
  "Convert to Solid" before a save hid nothing and the warning's own text ("hide it for good")
  was false there — the save path never fires `guard_redactions` now, `Purpose::Save` and its
  "Save Anyway" string are removed (nothing constructs it any more; Copy/Share/Batch, tasks
  2.3–2.5, will add their own `Purpose` variants when they're built, reusing `guard_redactions`
  the way Export already does). The live test's warning+Convert coverage moved onto Export
  (the only path it still applies to), followed by a Cancel of the export dialog that then
  opens, so no file is written by that part of the test.
- **Mockup 10 stays the one true version** of the redaction-warning dialog (horizontal buttons,
  warning icon, separate blur/pixelate wording) over add-core-annotation-tools mockup 19
  (stacked buttons, generic wording) and tasks.md 2.7's paraphrase of it — mockup 19 gets a
  small follow-up redraw to match, tracked separately, not touching anything else in that
  already-implemented change.
- **An export does not count as "saved".** Status, the unsaved dot and the autosave are
  unaffected by Ctrl+E — only writing `.annota` (quick save/Save As) clears them. An export is
  a one-way flattened copy, not the editable document; add-annotation-editor's capture-delta
  wording ("until saved or exported") is inherited phrasing to fix when that capability's own
  spec text is next touched, not a behaviour to match here.
- **Export dialog settings are not remembered across exports** within a session — every open
  of the dialog starts from the stored preferences, as built. Simpler and already covers the
  common case ("I always export as X") via the preference itself.
- **A save renames the written file's own title, and the session's displayed title, to the
  file's name** (`export::save_annota_named`, `Session::saved_as` — neither is an undo step:
  the written copy's metadata and the session name are not document state) but does **not**
  touch the live, in-memory `Document::metadata().title` the editor keeps working on —
  acceptable as built. Updating that would need its own `Command`/undo step (it is tracked,
  undo-history document state), which a save shouldn't silently create; until the document is
  next saved or explicitly renamed, an autosave taken meanwhile, or any export, still uses the
  pre-save title, even though the window and the file just written both already show the new
  one.

### Decided during apply (2026-10-31: task 2.3 clipboard + drag-out, task 2.4 Share)

**Redaction warning, extended** (task 2.7 continued). `redaction_warning::Purpose` gained three
variants, one per way out, each naming the middle button: `Copy` → **"Copy Anyway"** (the wording
tasks.md 2.7 already named), `Drag` → **"Drag Anyway"**, `Share` → **"Share Anyway"** (none of
the three is drawn; mockup 10 only shows "Export Anyway"). Share is one label for all three Share
items — the way out is the Share menu (the split button's dropdown tooltip is "Share"); per-item
labels ("Email Anyway", "Open Anyway", "Copy Anyway" for Copy to Folder) were considered and not
chosen, the last would also collide with the clipboard's. `Editor::guard_redactions` itself is
unchanged; every entry point calls it before doing anything else — Copy before rendering, Email/
Open With before the temporary PNG is written, Copy to Folder before its file chooser opens
(as Export's comes before its dialog). Batch export (2.5) is the last caller to add.

**Clipboard** (task 2.3, spec "Clipboard", mockup 07). `Ctrl+C` (window key controller, not an
application accelerator — text fields need it; text being edited on the canvas still copies its
selected text), the header's Copy and the narrow layout's main-menu "Copy to Clipboard" are one
action, `win.copy`: redaction warning, then the document flattened at 100 % as a premultiplied
`GdkMemoryTexture` (the tray's conversion) put on the clipboard through the existing
`capture::delivery::copy_to_clipboard` (holds the application on Wayland until another app owns
the clipboard). GTK offers a texture as `image/png` (plus other image types) to whoever pastes —
the "GdkClipboard texture + PNG mime" decision above, without encoding a PNG up front. Toast
"Image copied to clipboard", 3 s, default priority, no buttons (AdwToast's close button stays, as
mockup 07 draws). Copying is not saving: status and autosave unchanged.
- *With objects selected* — **not built anywhere**: index.md's Scenario table says that copies
  "the object (editor change)", but no change has an object clipboard (add-annotation-editor's
  context menu has a "Copy" entry wired to the same placeholder action, and Annota has no object
  paste). Until it exists, `Ctrl+C`, the context menu's Copy and the header's Copy all copy the
  flattened image, whatever is selected; the toast says "Image copied" so it is never ambiguous.
  Open question below.

**Drag-out** (task 2.3, spec "Drag and drop", decision D1, mockups 08/09).
- *Source*: a `GtkDragSource` (action COPY) on the header's `AdwWindowTitle`; tooltip "Drag image
  to another app" and the CSS class `annota-drag-source` only while a document is open. Hover:
  `alpha(currentColor, .07)` behind title + subtitle, radius 6, 34 px high (libadwaita's 12 px
  side padding is on the labels, so the fill covers it); during the drag `alpha(currentColor,
  .16)` (`annota-dragging`) — the kit's `--fill-flat-hover`/`--fill-flat-active`. The pointer
  stays the arrow. The title area no longer moves the window (the drag source claims the gesture
  first); the rest of the header bar still does, as the redline says.
- *Payload* (`share::drag_content`): one `GdkContentProvider` union of a `GdkFileList` with the
  temporary PNG — GTK serialises it as `text/uri-list` (and the portal file-transfer types for
  sandboxed targets) — and the same PNG bytes as `image/png`. Built in the drag source's
  `prepare`, synchronously: the document is flattened and encoded once at drag start (both
  formats share the bytes). A lazy provider would need a `GdkContentProvider` subclass; not done
  unless a large capture makes the start of a drag noticeably slow.
- *Drag image* (`share::ghost`): the flattened image as a 160 × 100 thumbnail (rendered at 2×,
  capped by two `AdwClamp`s, cropped to fill like the tray's drag image), radius 6, a 1 px edge
  instead of mockup 09's shadow (drag surfaces clip shadows — same choice as the tray's drag
  icon), 92 % opacity, the kit's accent badge "PNG" 6 px from the bottom-right. Hotspot (−6, −8):
  the thumbnail sits just below and right of the pointer, as drawn.
- *Redactions*: a drag cannot wait for an alert (the button is held; once it is released there
  is nothing to drop), so with blur/pixelate redactions `prepare` refuses this drag and — on the
  next idle, outside the gesture — shows the warning with **Drag Anyway**. Convert to Solid leaves
  nothing reversible; Drag Anyway records a per-document consent (`Inner::drag_consent`) that the
  next change to that document (any `Canvas::connect_changed`) takes back. Both end with the
  toast **"You can drag the image out now"** (not drawn; high priority, it answers what the user
  just did) and the next drag goes through.

**Hand-off files** (`src/export/handoff.rs`, library, no GTK). Drag-out, Email… and Open With…
need a real file another app reads *after* Annota handed it over. Each hand-off is
`$XDG_RUNTIME_DIR/annota/handoff/<unique>/<name>.png` (folders mode 0700; the runtime dir is
per-user, in memory and emptied at logout — the closest to "not persisted" (CLAUDE.md §7) that
another app can still read; GLib falls back to the cache dir without one). `<name>` is the
document name the header shows (`saving::file_stem`, "Untitled Capture" if empty), so the drop
creates "Checkout flow.png" (mockup 09) and the mail attachment has that name (mockup 14); the
unique folder keeps two hand-offs of one document apart. Not removed right after the hand-off: a
mail client may attach at send time (the Email portal passes an fd, but the document portal
behind it refers to the file) and a dropped file may be copied late. Every new hand-off removes
those older than 24 h (`handoff::MAX_AGE`).

**Share** (task 2.4, spec "Share", decision D1, mockups 01/14). The three items were disabled
stubs; their actions now belong to `Actions::document` (enabled with a document).
- *Email… / Open With…* go through the portals (`share::portal_sharer`, the only portal code of
  this phase): `ashpd` 0.11 `EmailRequest::attach(fd)` and `OpenFileRequest::ask(true)
  .send_file(fd)` (the system's app chooser), `identifier(None)` and `ensure_registered()` like
  the screenshot and colour-picker calls. No subject or body is set (nothing specified; only the
  attachment goes to the mail app). The editor reaches the portals through a `Sharer` (`Rc<dyn
  Fn(Target, PathBuf) -> future>`, like the eyedropper's `ScreenPicker`), so the live test runs
  everything around it with a stand-in. Errors → `share::ShareError` (sorted by
  `capture::portal::map_portal_error`): Cancelled is silent (the user closed the chooser);
  anything else → a high-priority toast "Could not open your mail app" / "Could not open the
  image in another app" (not drawn). Email shows mockup 14's toast "Opening your mail app with
  “Checkout flow.png” attached…" (high priority, no timeout) and dismisses it when the portal
  call returns, either way. Open With shows no toast of its own (the chooser is the feedback).
- *Copy to Folder…* is **not** a portal item of Annota's: the redaction warning, then GTK's
  `gtk::FileDialog::save` (title "Copy to Folder", button "_Copy", filter "PNG Images", name
  "<name>.png", starting in the `export-folder` preference) — GTK decides whether that is the
  FileChooser portal, exactly like Save As — then `export::export(doc, path, Png, 100 %)`
  (atomic write; no hand-off file). `.png` is appended if missing (like Save As's `.annota`).
  Toast "Copied as “Checkout flow.png”" with Show in Files (not drawn; worded like mockup 06's
  "Saved as" and the export dialog's "Exported as"); a failure shows the export banner's words
  (`saving::problem`) in a high-priority toast. Copying is not saving.

**Deviations from the mockups**: mockup 14's toast has no close button, libadwaita 1.5's
`AdwToast` always shows one; the drag image has a 1 px edge instead of a shadow (above). Not
drawn and decided here: "Copy Anyway"/"Drag Anyway"/"Share Anyway", "You can drag the image out
now", "Copied as …", the share failure toasts, the Copy to Folder chooser's labels.
`docs/design/add-export-share/index.md`'s Scenario table still lists "Quick save" among the
redaction warning's ways out (stale since the maintainer's 2026-10-31 decision above) — a
designer fix, not touched here.

**Spec deltas.** export-share: "Clipboard" (header Copy, mockup 07's toast, not a save), "Drag
and drop" (the title area instead of the pre-D1 drag handle; file name, `text/uri-list` +
`image/png`, tooltip, drag image), "Share" (+ Copy to Folder…, the hand-off toast, cancelling is
not an error; new scenarios "Open with" and "Copy to folder"). annotation-tools "Redaction export
warning": scenarios "Warning before copy", "Warning before a drag" (incl. the next drag going
through), "Warning before share".

**Tests.** Unit (no display): `export::tests` — the hand-off PNG equals a 100 % PNG export, is
named after the document, private (0700), never replaced by a second hand-off, no temporary
files, stale ones pruned and fresh ones kept, unwritable root → `PermissionDenied`, file names
cleaned; `ui::editor::share` — toast/tooltip texts of mockups 07/08/14, names and `.png`,
portal errors sorted (cancelled is not told), the runtime-dir root; `redaction_warning` — the
four middle-button labels. Live (`#[ignore]`, `copy_drag_and_share_on_a_live_display`): no
document → Copy/Share disabled, no drag; Ctrl+C on the canvas with nothing selected → warning
"Copy Anyway" → Cancel (clipboard empty, toast absent) → header Copy → Copy Anyway → clipboard
offers `image/png`, the pasted texture is 400 × 250 with the flattened pixels, document unchanged,
toast; drag → refused + warning → Drag Anyway → toast → payload: one file "Chat with Jonas.png"
under the hand-off root equal to the PNG export, `text/uri-list` and `image/png` (bytes equal),
ghost 160 × 100 with "PNG"; an edit revokes the consent → warning again → Convert to Solid →
drags go through; Email (stand-in portal) → hand-off toast while pending, gone after the answer,
attachment is the PNG; Open With cancelled → no toast, failing → toast; Copy to Folder's write
→ file equals the PNG export, toast with Show in Files, status unchanged, unwritable → false;
each Share item on a blurred document → "Share Anyway" warning → Cancel → no portal call. The
file chooser is never opened (Copy to Folder's chooser only behind a cancelled warning).
- *Running live tests without any portal*: plain `dbus-run-session` uses the session bus
  config, whose service directories let GTK's own settings lookup **activate the real
  `xdg-desktop-portal` binary on the private bus** (isolated from the maintainer's session, but
  still a portal process). This phase's live runs used `dbus-run-session --config-file=` a session
  config **without service directories** instead, so nothing can be activated — see
  `manual-checks.md`. Every other live test passes that way too, except
  `quick_styles_on_a_live_display`'s known theme issue (above).
- Real portal round-trips, real drops and real pastes are maintainer checks:
  `manual-checks.md` (new).

**Open questions for the maintainer** (do not block the next phases):
- *Ctrl+C with a selection*: copy the selected objects (needs an object clipboard + paste,
  an add-annotation-editor-scope follow-up) — or keep copying the whole image, as now? Until
  decided, the context menu's "Copy" on a selection copies the whole image too.
- *Drag with reversible redactions*: the drag that hits the warning is dropped and the user drags
  again after answering (no way around that in GTK). Acceptable, or should drags skip the alert
  and instead always convert/refuse?
- *Hand-off retention*: temporary PNGs stay up to 24 h in the runtime dir (until the next
  hand-off after that, or logout). Shorter (e.g. removed when Annota quits) risks a mail client
  that attaches at send time losing the file.
