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
- **Screens 01-14 use the pre-D1 header** (drag handle + Share button) instead of the header
  the editor actually has (Show Properties / Copy / Export / menu, decision D1). Maintainer
  wants 01-14 redrawn to match before (or alongside) implementing the rest of this change —
  tracked as its own task, see `tasks.md`.
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
