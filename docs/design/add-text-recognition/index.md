# Add text recognition — mockups

Grab text (whole image or selection), edit text in place, `text` capture mode, OCR preferences and the missing-Tesseract state. Sample content: `f-field-notes` (article), `a-harbor-backup` (typo fix), `e-relay-chat` (text capture), `d-tidewater-analytics` (no text found).

All screens: `src/NN-slug.html` → `png/NN-slug-{light,dark}@{1,2}x.png`. Render with `python3 tools/render-mockups.py docs/design/add-text-recognition`.

## Scenario → mockup

| Scenario / state | Mockup | Notes |
|---|---|---|
| Local recognition › Offline | [02-grab-text-dialog](png/02-grab-text-dialog-light@1x.png), [04-grab-whole-image-loading](png/04-grab-whole-image-loading-light@1x.png), [10-preferences-ocr-languages](png/10-preferences-ocr-languages-light@1x.png) | Every OCR surface carries the line *“Recognised on this computer with Tesseract. Works offline.”* (info icon, dim). No network state exists. |
| Grab text › Copy selection | [01-select-text-area](png/01-select-text-area-light@1x.png), [02-grab-text-dialog](png/02-grab-text-dialog-light@1x.png), [03-text-copied-toast](png/03-text-copied-toast-light@1x.png) | 01: Recognise Text tool (`O`) shows dashed text areas; the selected paragraph has handles and a sidebar preview. `Ctrl+Shift+C` opens 02 (review dialog, line breaks kept). Copy closes it and shows the toast 03. |
| Grab text (whole image) | [04-grab-whole-image-loading](png/04-grab-whole-image-loading-light@1x.png) | No selection + `Ctrl+Shift+C` = whole image. Loading state: spinner + determinate progress, Copy disabled. |
| Grab text (nothing found) | [05-no-text-found](png/05-no-text-found-light@1x.png) | Empty state inside the dialog; Copy disabled; sidebar says *No text areas found*. |
| Edit text › Fix typo | [06-edit-text-before](png/06-edit-text-before-light@1x.png), [07-edit-text-after](png/07-edit-text-after-light@1x.png) | Double-click a recognised line → inline field over the line (06). Sidebar shows the recognised text, matched font, estimated size, sampled text and background colour. Enter applies (07): original covered with sampled background, new text object selected, toast with Undo. |
| Text capture mode › Direct | [08-text-capture-select-region](png/08-text-capture-select-region-light@1x.png), [09-text-capture-notification](png/09-text-capture-notification-light@1x.png) | `annota --capture text`: region overlay with hint pill (08). After Enter: GNOME notification *Text copied* with the first line as preview and actions Show Text / Open in Editor (09). No editor window opens. |
| Language configuration (requirement text) | [10-preferences-ocr-languages](png/10-preferences-ocr-languages-light@1x.png) | Preferences › Text Recognition: follow system language, per-language checks, not-installed languages show the `apt` command. |
| Tesseract missing (package Recommends only) | [11-tesseract-not-installed](png/11-tesseract-not-installed-light@1x.png) | Warning banner below the header bar with the exact command and Copy Command; sidebar status page with Try Again. |
| Narrow (≤ 700 px) | [12-grab-text-narrow](png/12-grab-text-narrow-light@1x.png) | Grab-text dialog becomes a bottom sheet. |

## Screens

- [01-select-text-area](png/01-select-text-area-light@1x.png) · [dark](png/01-select-text-area-dark@1x.png)
- [02-grab-text-dialog](png/02-grab-text-dialog-light@1x.png) · [dark](png/02-grab-text-dialog-dark@1x.png)
- [03-text-copied-toast](png/03-text-copied-toast-light@1x.png) · [dark](png/03-text-copied-toast-dark@1x.png)
- [04-grab-whole-image-loading](png/04-grab-whole-image-loading-light@1x.png) · [dark](png/04-grab-whole-image-loading-dark@1x.png)
- [05-no-text-found](png/05-no-text-found-light@1x.png) · [dark](png/05-no-text-found-dark@1x.png)
- [06-edit-text-before](png/06-edit-text-before-light@1x.png) · [dark](png/06-edit-text-before-dark@1x.png)
- [07-edit-text-after](png/07-edit-text-after-light@1x.png) · [dark](png/07-edit-text-after-dark@1x.png)
- [08-text-capture-select-region](png/08-text-capture-select-region-light@1x.png) · [dark](png/08-text-capture-select-region-dark@1x.png)
- [09-text-capture-notification](png/09-text-capture-notification-light@1x.png) · [dark](png/09-text-capture-notification-dark@1x.png)
- [10-preferences-ocr-languages](png/10-preferences-ocr-languages-light@1x.png) · [dark](png/10-preferences-ocr-languages-dark@1x.png)
- [11-tesseract-not-installed](png/11-tesseract-not-installed-light@1x.png) · [dark](png/11-tesseract-not-installed-dark@1x.png)
- [12-grab-text-narrow](png/12-grab-text-narrow-light@1x.png) · [dark](png/12-grab-text-narrow-dark@1x.png)

## Redlines
- **Tool:** *Recognise Text*, icon `ocr-text`, single-key shortcut `O`, last slot in the tool strip after Crop.
- **Grab Text action:** `Ctrl+Shift+C` (menu label “Grab Text…”); works in every tool. With a selection → selection, else whole image.
- **Text areas:** fill `--accent-a12`, 2 px dashed (8/6) `--accent-bg` at 45 % opacity, radius 8 (image px). Selected area: fill `--accent-a25`, 3 px solid `--accent-bg`, four round handles. Toggle *Show text areas* (default on).
- **Grab Text dialog:** `AdwDialog`, 760 px wide (bottom sheet ≤ 700 px). Header: Cancel | “Grab Text” | Copy (suggested). Body: editable text view (`--fs-small`, line-height 1.65, view-bg, 1 px `--borders` inset), then row: language dropdown (200 px), check *Keep line breaks* (default on), counter “N lines · N words” (caption, dim). Footer note with `info` icon.
- **Toast:** “N lines copied to clipboard”, close button, no Undo.
- **Edit Text:** in-place field 2–3 px accent outline over the line; red wavy underline only for the spell-check hint. Sidebar group “Edit Text”: *Recognised* property row, Font dropdown (“Sans (matched)”), Size spin (estimated), *Text colour* and *Background* rows with small square swatch + hex (“Sampled · #7A7A7F”), buttons Revert + Done (suggested). Hint “Enter applies · Esc cancels”. Result is a normal text object (stays editable); toast “Text replaced” + Undo.
- **Text capture overlay:** standard region overlay (scrim 45 %), size label OSD under the selection, bottom pill: `ocr-text` icon, “Select text to copy”, separator, “Enter to confirm · Esc to cancel”.
- **Notification:** title “Text copied”, body “N lines from your selection are on the clipboard.” + first 50 characters in quotes, actions *Show Text* (opens the dialog standalone) and *Open in Editor*.
- **Preferences page “Text Recognition”:** groups Languages (switch *Follow system language*, checklist with `eng · installed`, not-installed rows dimmed with monospace `sudo apt install tesseract-ocr-<code>`) and Result (*Keep line breaks*, *Join words split by hyphens*, *Show text before copying* — off means `Ctrl+Shift+C` copies right away).
- **Missing Tesseract:** `AdwBanner` warning style: “Text recognition needs Tesseract. Install it with `sudo apt install tesseract-ocr`” + button *Copy Command*. Tool sidebar: compact status page “Text Recognition Unavailable” + *Try Again* (re-checks without restart).

Shared: editor shell = kit `template.html` (header 47 px, tool strip 52 px, properties 300 px), 6 px grid, Annota Coral `#ff5a4e` for annotation ink, Adwaita blue for UI accent. No accounts, cloud or network anywhere.
