# add-export-share — mockups

Source HTML: `src/`, renders: `png/<name>-{light,dark}@{1,2}x.png`. Render with
`python3 tools/render-mockups.py docs/design/add-export-share`.

| # | Screen | Light 1× |
|---|---|---|
| 01 | Header bar with drag handle + Share; Share menu open | [png](png/01-share-menu-light@1x.png) |
| 02 | Export dialog — PNG (default) | [png](png/02-export-dialog-png-light@1x.png) |
| 03 | Export dialog — JPG with quality, size dropdown open | [png](png/03-export-dialog-jpg-size-light@1x.png) |
| 04 | Export dialog — WebP lossy, error: folder not writable | [png](png/04-export-dialog-webp-error-light@1x.png) |
| 05 | Export dialog — PDF (vector annotations) | [png](png/05-export-dialog-pdf-light@1x.png) |
| 06 | Ctrl+S quick save toast with file name | [png](png/06-quick-save-toast-light@1x.png) |
| 07 | Ctrl+C clipboard toast | [png](png/07-clipboard-toast-light@1x.png) |
| 08 | Drag handle hover tooltip | [png](png/08-drag-handle-hover-light@1x.png) |
| 09 | Dragging the image into a Files window | [png](png/09-drag-into-files-light@1x.png) |
| 10 | Pixelated-redaction warning before export/share | [png](png/10-pixelate-warning-light@1x.png) |
| 11 | Batch: tray in selection mode, 3 selected | [png](png/11-batch-select-tray-light@1x.png) |
| 12 | Batch export dialog | [png](png/12-batch-export-dialog-light@1x.png) |
| 13 | Batch export done toast | [png](png/13-batch-export-done-light@1x.png) |
| 14 | Share → Email hand-off toast | [png](png/14-email-handoff-light@1x.png) |

All screens exist as `-dark@1x/2x`.

## Scenario → mockup

| Scenario | Mockup | Notes |
|---|---|---|
| WYSIWYG | 02–05 | Preview line "Pixel-identical to the canvas at 100 %". Pixel equality itself is verified by golden-image test. |
| Pattern | 06 | `Ctrl+S` on a new capture: no dialog, file written to the default save folder, title becomes the file name, subtitle "<folder> · Saved", toast. `Ctrl+Shift+S` opens the portal Save dialog (system UI, not drawn). |
| Paste elsewhere | 07 | `Ctrl+C` with no object selected (Select tool, no selection) copies the flattened PNG. With an object selected, `Ctrl+C` copies the object (editor change). |
| Drop into file manager | 08, 09 | Drag starts from the handle in the header bar; ghost = 160×100 thumbnail + "PNG" badge; drop creates `Checkout flow.png`. |
| Email | 01, 14 | Share → Email… exports a temporary PNG and calls the Email portal; toast while handing over. Open With… calls OpenURI (system app chooser, not drawn). |
| Batch | 11, 12, 13 | Three tray items selected → Export 3… → JPG → three files, toast. |
| (Redaction warning, from annotation-tools spec) | 10 | Shown on Export, Copy, Drag, Share and Quick save when the document contains blur or pixelate redactions. |

## Redlines

### Header bar (all editor screens)
Start: New Capture · linked Undo/Redo. End (left→right): **drag handle** (`drag-handle`, flat, tooltip "Drag image to another app") · **Share** (`share`, menu button) · Copy (`copy`, tooltip "Copy to Clipboard (Ctrl+C)") · **Export** (suggested, `Ctrl+E`) · main menu · close. This adds two buttons to the kit editor shell; everything else is unchanged.

### Share menu (01)
Popover 240 px: Email… · Open With… · — · Copy Image `Ctrl+C` · Save As… `Shift+Ctrl+S`. No other targets (no cloud, no upload).

### Export dialog (02–05)
- `AdwDialog` 560 px wide, header: Cancel | "Export Image" | **Export** (suggested). `Ctrl+E` opens it; `Enter` exports.
- Preview row: 160×100 thumbnail (radius 6, 1 px border) + name (heading), "<w × h px> · <format> · ≈ <size>" and "Pixel-identical to the canvas at 100 %" (12 px dim). Estimated size updates live.
- List 1: entry row "File name" (extension shown dim and not editable), row "Folder" + folder button (portal chooser). Defaults from preferences (`export-folder`, `default-export-format`).
- List 2: "Format" toggle group PNG | JPG | WebP | PDF, then format-specific rows:
  - JPG: "Quality" scale 1–100, default 85, value right-aligned.
  - WebP: "Compression" toggle Lossless | Lossy; Lossy adds Quality (default 80).
  - PDF: "Keep annotations as vectors" switch (on) + "Page size" dropdown (Fit to image, A4, Letter).
  - "Size" dropdown: 200 %, 100 %, 50 %, 25 % with pixel dimensions, separator, Custom Width… (03).
  - Raster formats: "Flatten annotations" switch shown **on and insensitive** with subtitle "Image formats always contain the flattened result. Save as .annota to keep them editable."
- Error (04): `.banner.error` at top of body, "No permission to write to this folder" + **Choose Folder…**; Export disabled until fixed. Same banner pattern for "A file with this name already exists" with **Replace** (destructive).

### Toasts (06, 07, 13, 14)
Centred over the canvas column (between tool strip and sidebar), 24 px above the tray.
- Quick save: “Saved as “Annota 2026-10-30 14-22-08.annota”” + **Show in Files** + close.
- Clipboard: "Image copied to clipboard" + close, 3 s.
- Batch: "3 images exported to <folder name>" + **Show in Files**.
- Email: "Opening your mail app with “<name>.png” attached…" (no buttons, dismissed when the portal returns).

### Drag (08, 09)
Handle 34×34 flat button; hover state = flat-hover fill + tooltip. During drag the handle stays pressed; content type `text/uri-list` (temporary PNG) + `image/png`. File name = document title + `.png`.

### Pixelate warning (10)
Alert 460 px: `warning` icon 32 px in `--warning`, heading "Make Redactions Solid?", body "<n> redactions use pixelate. Pixelated text can sometimes be reconstructed. Solid boxes hide it for good." (for blur: "use blur" / "Blurred text…"). Buttons: Cancel · Export Anyway · **Convert to Solid** (suggested). Convert is an undoable command, then the export continues.

### Batch (11–13)
- Tray "Select" button enters selection mode: bar shows Cancel (flat) · "<n> Selected" (centre) · Select All · **Export <n>…** (suggested). Selected thumbnails: 3 px accent ring + check (top-right, 18 px).
- Batch dialog 580 px, title "Export <n> Images": list of items (64×40 thumb, name, "→ <output file name>" updated live); Format / Quality / Size (same controls as single export); "File name pattern" entry row (default `%name`; tokens `%name`, `%n`, strftime) + info popover; Folder; "If a file exists" dropdown (Add a number / Replace / Skip).
