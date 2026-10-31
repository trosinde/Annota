# add-export-share — mockups

Source HTML: `src/`, renders: `png/<name>-{light,dark}@{1,2}x.png`. Render with
`python3 tools/render-mockups.py docs/design/add-export-share`.

| # | Screen | Light 1× |
|---|---|---|
| 01 | Header bar (decision D1); Share menu open from the Export split button's arrow | [png](png/01-share-menu-light@1x.png) |
| 02 | Export dialog — PNG (default) | [png](png/02-export-dialog-png-light@1x.png) |
| 03 | Export dialog — JPG with quality, size dropdown open | [png](png/03-export-dialog-jpg-size-light@1x.png) |
| 04 | Export dialog — WebP lossy, error: folder not writable | [png](png/04-export-dialog-webp-error-light@1x.png) |
| 05 | Export dialog — PDF (vector annotations) | [png](png/05-export-dialog-pdf-light@1x.png) |
| 06 | Ctrl+S quick save toast with file name | [png](png/06-quick-save-toast-light@1x.png) |
| 07 | Ctrl+C clipboard toast | [png](png/07-clipboard-toast-light@1x.png) |
| 08 | Hover over the title area (the drag source) with tooltip | [png](png/08-drag-handle-hover-light@1x.png) |
| 09 | Dragging the image into a Files window | [png](png/09-drag-into-files-light@1x.png) |
| 10 | Pixelated-redaction warning before export/share | [png](png/10-pixelate-warning-light@1x.png) |
| 11 | Batch: tray in selection mode, 3 selected | [png](png/11-batch-select-tray-light@1x.png) |
| 12 | Batch export dialog | [png](png/12-batch-export-dialog-light@1x.png) |
| 13 | Batch export done toast | [png](png/13-batch-export-done-light@1x.png) |
| 14 | Share → Email hand-off toast | [png](png/14-email-handoff-light@1x.png) |
| 15 | "Save Changes?" when closing a window or quitting with unsaved changes (task 2.6) | [png](png/15-close-confirmation-light@1x.png) |

All screens exist as `-dark@1x/2x`.

## Scenario → mockup

| Scenario | Mockup | Notes |
|---|---|---|
| WYSIWYG | 02–05 | Preview line "Pixel-identical to the canvas at 100 %". Pixel equality itself is verified by golden-image test. |
| Pattern | 06 | `Ctrl+S` on a new capture: no dialog, file written to the default save folder, title becomes the file name, subtitle "<folder> · Saved", toast. `Ctrl+Shift+S` opens the portal Save dialog (system UI, not drawn). |
| Paste elsewhere | 07 | `Ctrl+C` with no object selected (Select tool, no selection) copies the flattened PNG. With an object selected, `Ctrl+C` copies the object (editor change). |
| Drop into file manager | 08, 09 | Drag starts from the header bar's title area (title + subtitle, decision D1); ghost = 160×100 thumbnail + "PNG" badge; drop creates `Checkout flow.png`. |
| Email | 01, 14 | Export split button arrow (Share) → Email… exports a temporary PNG and calls the Email portal; toast while handing over. Open With… calls OpenURI (system app chooser, not drawn). |
| Batch | 11, 12, 13 | Three tray items selected → Export 3… → JPG → three files, toast. |
| (Redaction warning, from annotation-tools spec) | 10 | Shown on Export, Copy, Drag, Share and Quick save when the document contains blur or pixelate redactions. |
| (Save changes before closing, task 2.6 — document-format "Autosave and recovery" / "Closing keeps autosaves" MODIFIED) | 15 | Editor window closed (`Ctrl+W`, close button) or Annota quit (`Ctrl+Q`) while a document has unsaved changes → alert with Save / Discard / Cancel. Drawn after `Ctrl+W`, so the default response shows its focus ring. |

## Redlines

### Header bar (all editor screens)
Decision D1, as built in `data/ui/editor-window.blp` (redrawn 2026-10-31, task 1.3; screens 01-14
used to show this change's pre-D1 proposal with a drag-handle button and a separate Share button).
Start: New Capture · linked Undo/Redo. Title: `AdwWindowTitle` (document name / `W × H · Edited`),
also the drag source (see "Drag"). End (left→right), 6 px gaps:
- **Show Properties** — toggle, `sidebar-show-right-symbolic`, 34×34, tooltip "Show Properties (F9)"; pressed while the sidebar is shown (all screens here).
- **Copy** — `edit-copy-symbolic`, 34×34, tooltip "Copy to Clipboard (Ctrl+C)"; pressed state in 07.
- **Export** — `AdwSplitButton`, `.suggested-action`: main part label "Export" (tooltip "Export (Ctrl+E)", opens the Export dialog, pressed state in 02) + 26 px dropdown part with `pan-down-symbolic` (dropdown tooltip "Share", opens the Share menu, pressed in 01). The 1 px divider between the halves is the kit's `rgb(0 0 6 / 20%)` linked-suggested separator. Pressed halves use `--accent-bg-active`.
- **Main menu** — flat `open-menu-symbolic`, tooltip "Main Menu" (add-annotation-editor 17; it has no Share entries).
- Close.

No drag-handle button and no Share button in the header.

### Share menu (01)
The dropdown half of the Export split button opens a `GtkPopoverMenu` (menu model `share_menu`),
200 px, arrow centred on the dropdown half, shifted left to stay 6 px inside the window's right
edge. Items, one section, no accelerators: **Email…** (`win.share-email`) · **Open With…**
(`win.share-open-with`) · **Copy to Folder…** (`win.share-copy-to-folder`). No other targets (no
cloud, no upload). Copy Image and Save As… are no longer in this menu (Copy has its own header
button and `Ctrl+C`; Save As… is in the main menu, `Ctrl+Shift+S`). Drawn with Email… hovered.

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
Decision D1: the drag source is the header bar's **title area** (`AdwWindowTitle`, title +
subtitle, 34 px high, its natural width incl. its 12 px side padding — 140 px for "Checkout flow"
/ "1600 × 1000 · Edited"), not a separate button. The rest of the header bar's empty space still
moves the window as usual.
- Hover (08): `--fill-flat-hover` fill behind the title area, radius 6 (`--radius-s`); tooltip
  "Drag image to another app", centred below the title area. Pointer stays the default arrow.
- During the drag (09): the title area keeps the pressed fill `--fill-flat-active`.
- Ghost = 160×100 thumbnail of the flattened image (radius 6) + "PNG" badge bottom-right.
- Content: `text/uri-list` (temporary PNG) + `image/png`. File name = document title + `.png`.

### Pixelate warning (10)
Alert 460 px: `warning` icon 32 px in `--warning`, heading "Make Redactions Solid?", body "<n> redactions use pixelate. Pixelated text can sometimes be reconstructed. Solid boxes hide it for good." (for blur: "use blur" / "Blurred text…"). Buttons: Cancel · Export Anyway · **Convert to Solid** (suggested). Convert is an undoable command, then the export continues.

### Batch (11–13)
- Tray "Select" button enters selection mode: bar shows Cancel (flat) · "<n> Selected" (centre) · Select All · **Export <n>…** (suggested). Selected thumbnails: 3 px accent ring + check (top-right, 18 px).
- Batch dialog 580 px, title "Export <n> Images": list of items (64×40 thumb, name, "→ <output file name>" updated live); Format / Quality / Size (same controls as single export); "File name pattern" entry row (default `%name`; tokens `%name`, `%n`, strftime) + info popover; Folder; "If a file exists" dropdown (Add a number / Replace / Skip).

### Save changes before closing (15)
- `AdwAlertDialog` (kit `.dialog.alert`, 372 px, horizontal responses, 12 px gap, buttons 40 px high), over the editor window it belongs to. No icon (GNOME convention for this alert).
- Heading **"Save Changes?"**.
- Body, document saved before and edited since: "“<document title>” has unsaved changes. If you close without saving, they will be lost." Never saved (new capture, pasted image, image opened from a file and changed): "“<document title>” has never been saved. If you close without saving, it will be lost." `<document title>` = the header-bar title.
- Responses, left → right, ids in brackets: **Cancel** (`cancel`, default appearance) · **Discard** (`discard`, destructive appearance — red label on the neutral fill, as in add-annotation-editor 14) · **Save** (`save`, suggested). Mnemonics `_Cancel`, `_Discard`, `_Save`.
- Default response `save` (focused when the dialog opens, `Enter`); close response `cancel` (`Esc`). The focus ring is only visible when the close was triggered by keyboard (`Ctrl+W`/`Ctrl+Q`), as drawn.
- When it appears: closing the editor window (close button, `Ctrl+W`) or quitting (`Ctrl+Q`, main menu Quit) while a document of that window has unsaved changes (subtitle "· Edited" / "· Not saved", unsaved dot in the tray's Open group). Documents without unsaved changes close silently.
- Save = exactly what `Ctrl+S` does (quick save: overwrite the document's `.annota`, or for a never-saved document write it to the default save folder without a file chooser, mockup 06); on success the autosave is removed and the window closes. If saving fails the window stays open with the document unchanged and the save error is shown as for `Ctrl+S`.
- Discard closes without saving and removes the document's autosave. Cancel (or `Esc`) keeps the window and every document open, nothing changes.
- Several unsaved documents in one window: one alert per unsaved document, in the tray's Open order; before each alert the editor switches to that document so it is visible behind the dialog. Cancel at any point stops the whole close/quit (documents already saved stay saved, already discarded ones are closed). Confirmed by the maintainer 2026-10-31 over the alternative (one alert listing all unsaved documents with checkboxes, as GNOME Text Editor does).
- Labels are gettext strings; German: "Änderungen speichern?", "Abbrechen", "Verwerfen", "Speichern".
