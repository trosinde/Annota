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

### Decided during apply (2026-11-01: PDF Size only sets the resolution)
Maintainer decision, reversing the earlier state, where `page_layout` got pixels already multiplied
by the export scale: 200 % doubled a Fit-to-image page and shrank by a different amount on
A4/Letter.
- **The physical size depends only on the canvas pixels and `Metadata::scale_factor`.**
  `encode_pdf` passes canvas pixels to `page_layout`; the export scale is not an input of the
  layout. 96 px per inch and the division by the device scale (decided 2026-10-31) are unchanged.
- **The bitmap layer keeps its higher pixel density, vectors stay vectors.** 200 % embeds twice the
  pixels per side in the same place on the page (checked with `pdfinfo` and `pdfimages -list`: 135 ×
  75 pt at both, 180 × 100 px at 96 ppi against 360 × 200 px at 192 ppi).
- **Output pixels map to points per axis**, not by one factor: rounding the output size can change a
  side's proportion (101 × 51 px at 33 % is 33 × 17 px), and the bitmap must still cover the image's
  place with no blank strip. The distortion is under one output pixel.
- **Mockup 05 stays as drawn:** it shows only the 100 % case, where nothing changes.
- **The dialog's summary is unchanged:** "<w × h px> · PDF" is the bitmap layer's resolution at that
  Size, which is true, but it says nothing about the page.
- Tests in `src/export/tests.rs`: `pdf_is_one_page_of_the_image_size`,
  `pdf_size_changes_the_resolution_not_the_page`, `pdf_size_keeps_the_images_place_on_paper`,
  `pdf_bitmap_fills_the_page_when_a_size_rounds`.

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
  Open question below. *(Superseded: built as task 2.4a, see "Decided during apply (2026-10-31:
  task 2.4a object copy/paste)" below.)*

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

### Decided during apply (2026-10-31: maintainer follow-ups on tasks 2.3/2.4)
- **Object copy/paste will be built, as task 2.4a** — over the "leave it for later"
  recommendation (it's really an add-annotation-editor-shaped feature, but the maintainer wants
  it in this change since the original mockup table assumed it already existed). `Ctrl+C` with
  a selection puts the object(s) on the clipboard in a self-contained Annota-specific mime type
  (geometry, properties, embedded resources — the same shape `.annota` already serializes, so
  a copy survives the source document closing). **Correction to the first-pass scoping**: the
  existing "paste a clipboard image as a new document" (`capture::import::paste`,
  `app.paste-image`) turns out to be bound to **Shift+Ctrl+V**, not Ctrl+V — checked against
  `src/application.rs` after writing the first version of this note, which wrongly assumed a
  Ctrl+V collision to work around. Plain Ctrl+V is free, so the new editor-scoped `win.paste`
  simply binds it: it checks for the Annota mime type first and inserts into the *currently
  open* document (any open document, not only the one the objects were copied from) as one
  undo step, offset like a duplicate (10px convention); without that mime type it falls back to
  the same behaviour as `app.paste-image`. `Shift+Ctrl+V` is untouched. See `tasks.md` 2.4a.
- **The drag/warning interaction is acceptable as built**: a drag with reversible redactions
  doesn't start on the first attempt (GTK can't hold a drag open behind an alert) and shows the
  warning instead; a second drag attempt after Drag Anyway/Convert to Solid goes through. No
  change needed — this is a real GTK/libadwaita constraint, not a design choice to revisit.
- **Hand-off retention stays 24h pruning, not deleted on Annota quit** — as built. Deleting on
  quit risks breaking a mail client that reads the attachment lazily at send time; 24h is
  generous for normal use, and `$XDG_RUNTIME_DIR` is cleared on logout/reboot regardless.

### Decided during apply (2026-10-31: task 2.4a object copy/paste)

**Copy with a selection** (`win.copy`: `Ctrl+C`, the header's Copy, the narrow main menu's "Copy
to Clipboard", the context menu's Copy — one action, as before). With a selection, the selected
top-level objects (a group counts as one, its members travel inside it) go on the clipboard
under **`application/x-annota-objects`** — Annota-private (`x-`). Held like the image copy
(`delivery::copy_content_to_clipboard`, the same Wayland app hold). *(Superseded the same day,
see "Decided during apply (2026-10-31: selection copy includes the image)" below: as first
built, Annota's type was the only one offered, with no redaction warning and the toast "Object
copied to clipboard" — which made `Ctrl+C` right after drawing (the new object stays selected)
paste nothing into other apps. Now the flattened image is offered beside it, behind the
warning, with the toast "Image and object copied to clipboard".)*

**Payload shape** (`annota::document::clipboard`, library, no GTK) — a cut-down `.annota`, not a
new format: a ZIP with `objects.json` = `{"format", "requires", "generator", "objects": [...]}`
(document.json's versioning keys and its `objects` list, each node written by the same
`registry::node_to_json` a save uses — ids, type tag, fields, unknown data merged back) and the
used embedded images as `images/<n>.png` under the source document's ids, PNG-encoded and
stored as in `.annota`. No `original.png`, canvas or metadata. Reading uses
`registry::node_from_json` and `archive`'s own `read_entry`/size limits/version check
(`archive::check_format`, factored out of `read_document_json`), so a copied object round-trips
exactly like a saved-and-reopened one (unit test: the payload's nodes give the same JSON as
`document_json` of the source). The payload's format is the source document's
(`archive::written_format`: its file's newer minor version, if any). The bytes are the content
provider's own: nothing in a paste refers back to the source document, which may be closed (or,
across processes, its Annota gone).

**Paste** (`win.paste`, **`Ctrl+V`**; `editing::ops::paste`). A window key like `Ctrl+C` — not an
application accelerator, because text fields and text being edited on the canvas paste text
with it (the canvas's text editing takes it first, as before); `Shift+Ctrl+V` stays
`app.paste-image`'s accelerator, unchanged, and never reaches the window's key controller. In
the `document` action group (enabled with a document). Without a document, `Ctrl+V` does
nothing; `Shift+Ctrl+V` still pastes an image.
- *Which*: `share::paste_kind` — Annota's type anywhere in the clipboard's formats wins; otherwise
  `capture::import::paste(app)` is called, the very function `app.paste-image` runs (so an image
  opens as a new document with its own toast, no image gives "The clipboard does not contain an
  image") — no duplicated logic.
- *Into*: the document `Ctrl+V` was pressed in (by session uid; if it closed while the clipboard
  answered, nothing happens). One undo step **"Paste <object>"** ("Paste Arrow", "Paste 2
  Objects"), the same shape as Duplicate: one `Command::Insert` on top, in the copied stacking
  order, every node (and group member) with a fresh id of the target, offset by
  `DUPLICATE_OFFSET` (10 px) — always, also into another document — and **the pasted objects
  become the selection**, as Duplicate's copies do. The same clipboard pasted again lands at the
  same +10 px spot (it does not cascade like repeated Duplicate does, since the clipboard keeps the
  original position); not changed unless asked.
- *Embedded images*: resource ids belong to one document, so every pasted image gets a fresh
  `images/<n>.png` of the target — unless the target already has the very picture (same size and
  PNG encoding; e.g. pasting back into the source document), which is then shared, as
  Duplicate shares it. New `Annotation::rename_resources` (default no-op; `Image`, `Group`
  recursively, `UnknownObject` rewriting its resource strings) points the objects at them.
- *Failure*: a payload that does not read (damaged, or a newer *major* format) → high-priority
  toast "The copied objects could not be pasted" / "These objects were copied from a newer
  Annota and cannot be pasted here" (neither drawn); nothing changes.

**Forward compatibility — kept, same rule as `.annota`.** Not overkill, because it costs nothing
extra (the payload is read by the same `node_from_json` that reads files) and because the case is
real without two Annota versions running: a document opened from a newer-minor file carries
residual keys on its objects, and copying such an object into another document must not strip
them (it would, if the clipboard had its own lossy shape). For payloads from a genuinely newer
Annota (two versions sharing one session clipboard, e.g. a Flatpak and the .deb): newer major →
refused before anything else is read; newer minor → unknown keys at any depth stay on each node
as its residual, unknown types and unreadable known types become `UnknownObject`s (with the
images they appear to use; `UnknownObject::renumber_nested` gives node-shaped children inside new
ids so pasting one twice cannot repeat them). Pasting such a payload into a document of an
older minor also sets the document's minor version (new **`Command::SetFormatMinor`**, batched
with the Insert, undone with it): an unreadable known-type object is only legal in a file that
says it is of the newer minor version — saved as "1.0" it would make the file fail to reopen.
Unknown keys at the payload's top level (beside `objects`) have no counterpart in a document
and are dropped (documented in the module).

**Deviations / not drawn.** No mockup draws object copy or paste: the two toasts, the two failure
toasts and the "Paste <object>" undo label are decided here. Shortcuts window: new row
**"Paste" `Ctrl+V`** after "Copy image" in "Document" (bootstrap-app-shell mockup 07 has no
paste row; "Copy image" is kept as drawn — accurate again since the fix below: `Ctrl+C`
always copies the image, with a selection the objects too). The context menu gets no "Paste" entry (none drawn).

**Spec deltas.** export-share "Clipboard": the header's Copy is the same action, so "with no
object selected" applies to both; "Object copy and paste": `Ctrl+C` *or the header's Copy*, no
redaction warning, the toast *(superseded: both types, behind the warning — see below)*.

**Tests.** Unit (lib): `document::clipboard` — round trip equals the saved document's object JSON,
images included once; stacking order and selection subset, no payload for nothing; groups with
members; newer-minor payload keeps unknown keys, unknown types and unreadable known types (and
the same payload as "1.0" fails); newer major refused; damaged payloads fail; a payload outlives
its document. `editing::ops` `paste_tests` — scenario "same document" (offset 10 px, one undo step,
selection, pastes again); same command, selection and offset as Duplicate; scenario "different
document after the source closed" (source dropped, target's own `images/1.png` untouched, pasted
pictures under `images/2.png` equal to the source's, the target saves and reopens, one undo
removes both); pasting where the picture is shares it; pasted groups get unique member ids;
newer-minor objects paste, the document's format becomes 1.3 and reopens, undo restores 1.0;
unknown objects get new nested ids and image names; empty clip. Bin: `share` — `paste_kind`
(Annota's type wins, images/text/nothing fall back, look-alikes do not count; scenario "Plain
image"), toast texts; `labels` — "Paste Arrow", "Paste 2 Objects". Live (`#[ignore]`,
`object_copy_and_paste_on_a_live_display`, Xvfb's clipboard, no portal, isolated bus): no
document → `paste` disabled; Ctrl+C on a selected arrow in a document with a pixelate redaction
→ no warning, only `application/x-annota-objects` offered (no `image/png`), toast *(flipped by
the fix below)*; Ctrl+V → a
4th object, selected, offset 10 px, original unchanged, "Paste Arrow", undo; header Copy of the
picture → second document with its own `images/1.png` → Ctrl+V → `images/2.png` with equal
pixels, saves and reopens; the first document closed → Ctrl+V still pastes ("Paste Image"); a
damaged payload → toast, nothing pasted; a texture on the clipboard → Ctrl+V opens a new 64 × 48
document, the other unchanged; Shift+Ctrl+V is not taken by the window. All 14 live tests pass
(each in its own process, `isolated-session.conf` without service directories). Cross-process
paste (two Annota processes, or after Annota quit on Wayland) is a maintainer check:
`manual-checks.md` rows 9-10.

### Decided during apply (2026-10-31: selection copy includes the image)

**The bug** (open question left by task 2.4a): a newly drawn object stays selected, so `Ctrl+C`
right after drawing copied only `application/x-annota-objects` — pasting into a chat or an email
gave nothing, although the toast said "copied". **Maintainer decision (2026-10-31): Copy with a
selection SHALL put both types on the clipboard.** Resolved and built as follows.

**Payload** (`share::selection_content`): one `gdk::ContentProvider::new_union` (the pattern of
`share::drag_content`) of (1) the selected objects' payload under `application/x-annota-objects`
(`objects_content`, unchanged: `document::clipboard::encode`) and (2) the **whole** document
flattened at 100 % as a `GdkTexture` (`for_value`) — the very texture Copy without a selection
puts there, which GTK serialises as `image/png` (and other image types) for other apps. Not a
crop of the selected objects: right after annotating, the intent is "share the finished
picture". Annota's `Ctrl+V` is unchanged (`share::paste_kind`: Annota's type anywhere in the
formats wins), so pasting back still pastes the objects; `document::clipboard` and
`editing::ops::paste` are untouched. If either half cannot be made (an embedded image that does
not encode, a document too large to draw), nothing is copied and the respective failure toast
shows — half a copy would bring the trap back.

**Redaction warning**: `Editor::copy` now calls `guard_redactions(Purpose::Copy, …)` for both
cases, before anything is encoded — the image leaves Annota either way. Scope is the **whole
document** (`redactions::reversible`, as for Copy without a selection), since the image half is
the whole document; a selection without redactions in a document with a blur still asks. The
selection is captured at `Ctrl+C`; Convert to Solid (one `Command::Replace` that keeps the ids)
runs before the copy proceeds, so both halves are taken from the converted document — a selected
pixelate redaction is copied as solid, and the image shows it solid. Copy Anyway copies both
unconverted; Cancel copies nothing.

**Toast**: "Image and object copied to clipboard" / "Image and <n> objects copied to clipboard"
(de: "Bild und Objekt in die Zwischenablage kopiert" / "Bild und {n} Objekte in die
Zwischenablage kopiert"; not drawn; 3 s, default priority as before). It names the image first —
what another app gets — and still differs from mockup 07's "Image copied to clipboard", so it is
clear the objects are there for Annota as well. The shortcuts window's "Copy image" row is
accurate again.

**Unchanged**: Copy without a selection, drag-out, Share, Export, `Ctrl+V`/`Shift+Ctrl+V`.

**Spec deltas.** export-share "Object copy and paste": both types, after the redaction warning,
the new toast; new scenarios "Paste the copied selection into another app" and "Warning before
copying a selection" (incl. Convert to Solid applying to both halves). annotation-tools
"Warning before copy": "with or without objects selected".

**Tests.** Bin unit (`share`): `copy_with_a_selection_offers_the_objects_and_the_whole_image` —
the union offers Annota's type and the texture type, `paste_kind` still says Objects, the texture
half is the flattened document's texture, the object half is the payload byte for byte (GTK's
texture → PNG serialiser needs a display, so `image/png` itself is checked live); toast texts.
Live (`object_copy_and_paste_on_a_live_display`, assertion flipped from "no warning, no
`image/png`"): Ctrl+C on the selected arrow in the document with a pixelate redaction → the
warning with Copy Anyway → Cancel → clipboard empty, no toast → again → Copy Anyway → document
unchanged, both types offered, Annota's half decodes to the one arrow, the `image/png` read back
from the clipboard is 400 × 250 with the flattened document's pixels (pixelated block, arrow,
picture), the new toast; Ctrl+V still pastes the arrow; the redaction selected → Ctrl+C →
Convert to Solid → "Convert Redaction to Solid" is the undo step, the copied object is a solid
redaction and the `image/png` equals the converted document's flattening; the picture's header
Copy then shows no warning and offers both types. All other live tests unchanged and passing
(each in its own process, isolated bus without service directories) except
`quick_styles_on_a_live_display`'s known theme issue (above). Manual check rows 8 and 9 updated
(paste the selection copy into another app → the whole image).

### Decided during apply (2026-10-31: task 2.5 batch export, task 2.7 completed)

**Entry point — built against the real tray, not mockup 11's drawing.** Mockups 11/13 still draw
the pre-add-annotation-editor tray (one "Recent Captures" row, captions under the tiles); the
built tray has an "Open" and a "Recent" group, each with its own 15 px label, and Open Library /
Hide Tray on the right (add-annotation-editor's redline). Kept from the mockups: **"Select"** at
the right end of the label row (mockup 13; a flat 22 px button with a 13 px bold label, overlaid
on the scroller's top-right, tooltip "Select Images to Export", shown only while the tray has
tiles), and in selection mode **a bar in the label row's place** (mockup 11): Cancel (flat) ·
"<n> Selected" (centred, `GtkCenterBox`) · Select All · **Export <n>…** (suggested, disabled at
0). The "Open"/"Recent" labels and the unsaved dots fade out (CSS `opacity: 0`, so nothing
moves); the current document's ring is dropped; selected tiles get the 3 px accent ring and the
18 px check (top-right; an empty white box on unselected tiles, as drawn). Not drawn and decided
here:
- *Both groups are selectable.* Open documents as they are (text being typed is finished
  first); recent files (the save folder's `.annota`/`.png`, the tray's stand-in for the capture
  library) are read with `session::load` when Export <n>… is pressed — one that cannot be read
  gets the editor's existing "Could not open “…”: …" toast and is left out.
- *Order* is the tray's (Open left to right, then Recent), not the click order: the dialog's list
  and `%n` follow what the user sees.
- A click in selection mode selects instead of opening/switching; dragging a tile onto the
  canvas still works. Cancel, **Esc** (window key, before the canvas gets it), hiding the tray
  (`show-tray`, the narrow layout) and a successful export leave selection mode and forget the
  selection; a failed export or the dialog's Cancel keep it, to try again. Tiles that go away
  (document closed, file gone) leave the selection; without tiles there is no selection mode.
  Tiles carry the accessible `checked` state while selecting.

**The dialog** (`src/ui/editor/batch.rs`, `data/ui/batch-export-dialog.blp`; mockup 12): AdwDialog
580 px, Cancel | "Export <n> Images" (ngettext) | **Export** (suggested, default widget), then
the list of documents (64 × 40 thumbnail radius 4, name, "→ <file>" updated on every change),
Format / Quality / Size, then File name pattern (monospace, info button with a popover listing
the tokens), Folder (folder button: GTK's folder chooser) and If a file exists (Add a number /
Replace / Skip). As in "Export Image", every open starts from the preferences
(`default-export-format`, `export-folder`) and nothing is remembered.
- *Shared with the export dialog, not duplicated*: `export_dialog::Encoding` (format, JPG/WebP
  qualities, WebP lossy, PDF options → `export::Format`, the per-format size limit) and
  `export_dialog::FormatRows` (the Format toggle, Compression, Quality, Keep annotations as
  vectors, Page size: shows an `Encoding`, reports edits) — the same object ids in both `.blp`
  files. The export dialog's `Form` now holds an `Encoding` (behaviour unchanged, its tests
  adjusted).
- *Size* is an `AdwComboRow` of the presets "200 %" … "25 %" without pixel sizes (they differ
  per document) and without Custom Width… (a width for documents of different widths would be
  a different feature). *No "Flatten annotations" row* (not drawn in 12).
- *Banner* (not drawn in 12, the export dialog's): "No permission to write to this folder" +
  Choose Folder…, or a document too large for the format at this size; Export disabled. Export
  is also disabled when every document would be skipped.
- *Pattern* (`batch::expand_pattern`): `%name` (the name the tray shows), `%n` (number in the
  selection, zero-padded to the selection's size: 1–9, 01–12), strftime for the rest (glib's
  `DateTime::format`, like the `filename-pattern` preference), `%%` a literal `%`. `%name`/`%n`
  are replaced before strftime, so a `%` in a document's name stays literal. The time is the
  moment the dialog opened — the names shown are the names written. `/` becomes `-`; a pattern
  that leaves nothing falls back to the document's name.
- *If a file exists* (`batch::plan`) is about files already in the folder. Names **inside one
  batch never collide** whatever it says (a later document gets " 2", " 3" …, quick save's
  numbering): a pattern without `%name`/`%n`, or two documents of the same name, still give
  every document its own file, and Replace never replaces what the batch itself just wrote.
  The lines say "→ name.jpg · replaces the existing file" / "Skipped · “name.jpg” already
  exists" (not drawn).
- *Writing*: one document after the other on the main loop with a yield in between (the window
  redraws); the dialog cannot be closed meanwhile and closes when done. The folder is created if
  missing. A document that fails is counted and the others go on. Exporting is not saving.

**Done** (mockup 13): "3 images exported to Exports" (ngettext; the folder's own name) with
**Show in Files** (opens the folder with the first file selected), and selection mode ends. If
some failed (not drawn): a high-priority toast "2 images could not be exported: <the export
banner's words>" and the selection stays.

**Redaction warning for a batch — decided: once for the whole selection** (task 2.7's open
call). One "Make Redactions Solid?" before "Export <n> Images" opens, its body counting the blur
and pixelate redactions of every selected document together (mockup 10's text, no new strings),
middle button **Export Anyway** (`Purpose::Export` reused — it is an export). Cancel exports
nothing and keeps the selection; Convert to Solid converts in every selected document: an open
document gets its own "Convert Redaction(s) to Solid" undo step (one per document — undo is per
document), a recent file only in the copy read for this batch (`make_solid_copy`; no editor holds
it, so no undo step, and its file is never written). Per-document alerts were rejected: a batch
of ten would ask up to ten times, and the alert's choice ("make them solid before they leave
Annota") is the same question for the whole batch. Implementation: `Editor::ask_redactions` is
the alert half of `guard_redactions` (which keeps counting the current document only), so every
way out goes through the same alert. **Task 2.7 is complete**: Export, Copy, drag-out, Share and
batch export; never a save.

**Deviations / not drawn**: the tray's actual layout (above); "Select Images to Export", the
Replace/Skip lines, the failure toast, the banner, the Size entries without pixel sizes. The info
button uses `dialog-information-symbolic` like every other info button in Annota (an "i" in
Ubuntu 24.04's adwaita-icon-theme 46; adwaita-icon-theme 50 draws it as a light bulb).

**Open question for the maintainer**: mockups 11 and 13 should be redrawn against the real tray
(Open/Recent groups, no captions) now that the entry point is built — the follow-up the
2026-10-31 "other staleness" note foresaw for 11/12/13; 12 matches as built.

**Spec deltas.** export-share "Batch export" rewritten (Select, both groups, Export <n>…, the
pattern tokens, If a file exists, the toast) with scenarios "Batch" (+ toast, end of selection
mode), "Selection mode", "File name pattern", "File exists". annotation-tools "Redaction export
warning": "(in a batch export once for the whole selection)" and scenario "Warning before batch
export".

**Tests.** Unit (`ui::editor::batch`): pattern tokens (`%name`, padded `%n`, strftime, a name's
own `%`, `%%`, glib dropping a trailing `%`), stems (`/`, fallbacks), Add a number / Replace /
Skip, no collisions inside a batch for every policy and Replace never hitting the batch's own
file, the form's plan per format and pattern (scenarios "Batch", "File name pattern"), problems
(no permission, one document too large for WebP at 200 %), the texts of mockups 11–13.
`export_dialog` tests unchanged in substance. Live (`#[ignore]`,
`batch_export_on_a_live_display`, isolated bus, temporary folders, `export-folder` set through
the preference — the folder chooser is never opened): Select → bar, "0 Selected", Export 0…
disabled; clicks select tiles of both groups without switching or opening; Select All and tray
order; Esc, Cancel and hiding the tray end selection mode; Export 3… → one warning "2
redactions use blur or pixelate" with Export Anyway → Cancel (nothing changed, selection kept) →
again → Convert to Solid (open document's undo step, recent file's bytes unchanged) → "Export 3
Images" with the three "→ …png" lines → JPG (Quality shows, lines follow) → a file in the way:
Skip / Replace / Add a number lines → pattern `%n %name` → Export: three JPGs (the existing file
untouched, the recent file's JPG byte-equal to its converted copy's encoding), toast with Show
in Files, selection mode over, documents still unsaved; a read-only folder → banner, Export
disabled, Cancel keeps the selection.
- **Found while comparing screenshots** (not a regression): the maintainer's
  `~/.config/gtk-4.0/gtk.css` (a symlink to the Everforest-Green-Dark theme) is loaded by GTK at
  `USER` priority, above Annota's own stylesheet, so live tests run with the real
  `XDG_CONFIG_HOME` render — and assert — that theme's buttons, labels and colours instead of
  Annota's. That is the cause of the long-standing `quick_styles_on_a_live_display` "theme issue"
  noted above: it passes with `XDG_CONFIG_HOME` pointed at an empty folder and fails with the
  real one. Live tests are now run that way (`manual-checks.md`).

### Decided during apply (2026-10-31: task 2.6 save changes before closing)

**The alert** (`data/ui/save-changes-dialog.blp`, `src/ui/editor/closing.rs`; mockup 15 and its
redline): an `AdwAlertDialog`, heading "Save Changes?", no icon, responses `cancel` "_Cancel" ·
`discard` "_Discard" (destructive) · `save` "_Save" (suggested), default `save` (Enter), close
response `cancel` (Esc). Body (`closing::body`): "“<title>” has unsaved changes. If you close
without saving, they will be lost." for a document that has its own `.annota` file (opened from
or saved to one, or restored from an autosave that belongs to one — `Session::source`), "“<title>”
has never been saved. If you close without saving, it will be lost." otherwise; `<title>` is the
header bar's title (`Session::name`).
- *Width*: `width-request: 372` (the kit's `.dialog.alert`). Without it AdwAlertDialog is 300 px
  wide and stacks the three responses vertically; it ignores `content-width`. With it the
  responses sit side by side as drawn. libadwaita 1.9 (this dev container) draws the destructive
  response as red text on a red-tinted fill; libadwaita 1.5 (Ubuntu 24.04) as red text on the
  neutral fill, as drawn.

**Which documents are asked about** (`closing::needs_saving`): a document of a `.annota` file
and an image opened from a file only once they differ from that file (`HistoryInfo::dirty`; an
untouched image's pixels are on disk), a new capture / pasted image and a document restored from
an autosave always (they exist nowhere else). Text being typed is finished first (it is the
document's). Others close silently. (The tray's unsaved dot also shows on an untouched opened
image — unchanged, not this task's.)

**The sequence** (`Editor::confirm_close`, `ask_each`): the documents to ask about, in the tray's
Open order; for each, the editor switches to it (redline: visible behind its alert), then asks.
- **Save** = `quick_save_now`, exactly `Ctrl+S` (own file, or the filename pattern in the save
  folder — no file chooser); on success the autosave is gone (as for every save) and the
  document closes (`close_session`), then the next alert. **On failure** quick save's own toast
  (the export banner's words + Save As…) shows, the document stays open and unsaved with its
  autosave, and the whole close or quit stops — no further alert.
- **Discard** closes the document and removes its autosave (`close_session(uid, true)`, the same
  path as a new capture's Undo); its file, if any, is untouched.
- **Cancel** (or Esc, or closing the alert) stops the whole close or quit; documents saved or
  discarded so far stay so (they are closed), the rest stay open.
- After the last answer: the window closes (close path) or Annota quits (quit path). While the
  alerts are up another `Ctrl+Q` does nothing (`Inner::closing`); `Ctrl+W` / the close button
  closes the *alert* — libadwaita closes a window's open dialog instead of emitting
  `close-request` — which is Cancel.
- A dialog already open when quitting (Export Image, a warning, Restore Unsaved Captures?) is
  closed first (`force_close`; for Restore that keeps the autosaves). On the close path
  libadwaita does this itself: `Ctrl+W` with Export Image open first closes that dialog, a second
  `Ctrl+W` asks.

**Entry points.** Closing: the window's `close-request` (close button, `Ctrl+W` =
`window.close`) — stopped while there is something to ask, re-issued by `confirm_close` with
`close_confirmed` set once there is not. Quitting: `app.quit` (`Ctrl+Q`; GNOME Shell's "Quit"
activates the same action) now calls `editor::quit`, which asks first and calls
`Application::quit` only after the last answer. *Not built*: a "Quit" item in the editor's
main menu — the redline mentions "main menu Quit", but neither the built main menu nor
add-annotation-editor's mockup 17 has one (GNOME's HIG keeps Quit out of primary menus); open
question below.

**Autosaves on close — add-annotation-editor's "always keep" reverted.** An autosave is removed
only when its document is saved or the user discards it. Since every document that would lose
something is saved or discarded before the window closes (or Annota quits), `close_all` /
`Session::close` (a last autosave tick) only ever see documents without unsaved changes there,
and a stale autosave of one (changed, then undone) is removed by that tick. When Annota ends
**without** asking, its autosaves stay: killed, crashed or ended with the session (GTK does not
run `shutdown` then), the 30 s autosaves are what is left, as for any crash; if it still shuts
down normally without having asked (only `editor::quit` calls `Application::quit`, so this is
the window being destroyed some other way), the same last tick writes and keeps every unsaved
document's autosave. Either way "Restore Unsaved Captures?" offers them at the next start; that
dialog's text ("changes not saved in a file … Annota kept a copy") was
already neutral and stays. Doc comments in `session.rs`, `mod.rs` (module docs, `close_all`,
`shut_down`, `close_session`, `restore`), `application.rs` and `restore-dialog.blp` updated.

**Spec deltas.** New `specs/document-format/spec.md`: MODIFIED "Autosave and recovery" (also
removed by Discard in "Save Changes?"; new scenario "Discarded when closing"), "Closing keeps
autosaves" (kept name — still true for every autosave nobody was asked about — now: removed only
by Save/Discard in "Save Changes?", kept when Annota ends without asking, written once more on a
normal shutdown that did not ask; scenarios "Ended without
asking keeps the autosave", "Closed after saving or discarding"), "Recovery dialog" (left after a
crash or an ending without asking); ADDED "Save changes before closing" (scenarios "Document
saved before", "Never saved", "Several unsaved documents", "Save", "Save fails", "Discard",
"Cancel"). **Sourced verbatim** from add-annotation-editor's own `specs/document-format/spec.md`
delta (that change is not archived, so there is no `openspec/specs/document-format/` yet — the
same situation as task 2.4a's/2.7's `annotation-tools` delta copied from
add-core-annotation-tools'): unchanged scenarios copied word for word, changed text only where
the behaviour changed. **Archive order: add-annotation-editor before add-export-share** (`openspec
validate` notes archiving would refuse the MODIFIED delta without the base spec). `proposal.md`
lists `document-format` under Modified Capabilities.

**Tests.** Unit (`ui::editor::closing`): which documents are asked about (every origin, changed
or not), the two bodies, response ids → answers. Live (`#[ignore]`,
`save_changes_on_a_live_display`, isolated bus, temporary folders, no file chooser): five
documents (an edited `.annota` file, an unchanged one, two new captures, an untouched opened
image) → `window.close` → alert for the edited file, shown behind it, heading/body/default/close
response/appearances as in the redline → Esc → nothing changed → `Ctrl+Q` (`editor::quit`) → a
second quit does not stack alerts, `window.close` closes the alert (Cancel) → quit again → Save:
written over its own file, closed, autosave gone → next alert: the never-saved capture, its body
→ Cancel: quit stopped, saved document stays closed, others open with their autosaves → close
button → Discard: closed, autosave gone, nothing written → next document, save folder read-only
→ Save fails: quick save's toast with Save As…, no further alert, window and document open,
autosave kept → close again → Save: written under the filename pattern, then the window closes
by itself (the unchanged file and image are not asked about) and no autosave is left.
`editor_window_on_a_live_display`'s last part now checks the other half — Annota ending without
asking (`close_all`, as `shut_down` does) keeps the unsaved documents' autosaves with their last
change — since a plain window close asks now.

### Decided during apply (2026-11-01: maintainer follow-up on task 2.6)
- **A "Quit" item was added to both main menus** (`data/ui/editor-window.blp` and
  `data/ui/window.blp`, its own trailing section after About, GNOME HIG convention), over
  "leave it out" — matches what the redline assumed existed. It's the existing `app.quit`
  action (the same one `Ctrl+Q` already activates), so it goes through the same "Save Changes?"
  path with no further wiring needed. German "_Beenden" (po/de.po), mnemonic kept the way
  `_Preferences`/`_Keyboard Shortcuts`/`_About Annota` already do in the same menu.
