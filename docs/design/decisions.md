# Cross-change design decisions

Six designer agents produced the first mockup set in parallel (267 screens, October 2026).
This file resolves where their choices overlap or conflict. **It overrides individual
`index.md` redlines.** Designers and engineers read it before working on any change.

Status: decisions D1–D8 are set by the design lead; Q1–Q5 wait for the maintainer.

## Decided

| # | Topic | Decision | Affects |
|---|---|---|---|
| D1 | Editor header bar (end) | `Show Properties` toggle (F9) · Copy · **Export as split button** (main part exports, arrow opens Share: Email…, Open With…, Copy to Folder…) · main menu · close. The drag handle moves to the **title area**: dragging the title/subtitle drags the flattened image out. | add-export-share (01, 08, 09, 14), all editor screens |
| D2 | Tool strip | Order: Select · Arrow, Line, Pen, Shape, Text, Callout, Step · Highlighter, Redact, Magnify, Spotlight, Stamp · Crop · More Tools (Cut-out, Fill, Eraser, Magic Wand, Selection, Recognise Text). Tools of a later phase are **hidden** until their change ships. | all editor screens |
| D3 | Shortcut scope | Shortcuts are per window. Editor: `Ctrl+T` flatten selection, `Ctrl+Shift+T` flatten all. Library: `Ctrl+T` edit tags. | add-annotation-editor, add-library-organisation |
| D4 | Combine captures | `Ctrl+Shift+J` (join) in tray and library. `Ctrl+Shift+M` stays Smart Move. | add-templates, add-smart-editing |
| D5 | Additional shortcuts | Recognise Text `O` (More Tools), Effects panel `Ctrl+Shift+E`, Simplify `Ctrl+Shift+Y`, New from template `Ctrl+Shift+N`, Favourite `Ctrl+B` (library). Video editor: Play `Space`, Split `S`, Cut range `Delete`, Save frame `F`. | shortcuts window (bootstrap-app-shell 07) |
| D6 | Preferences structure | One preferences window with pages **General · Capture · Editor · Library · Text Recognition**. The Library and OCR pages drawn in their changes are pages of this window, not separate tabs. | bootstrap-app-shell, add-capture-library, add-text-recognition |
| D7 | Redaction warnings | Blur/pixelate warning fires on every way out of Annota: export, copy, drag, share, quick save. Faces default to **Solid**. | add-core-annotation-tools, add-export-share, add-smart-redaction |
| D8 | Kit additions needed | Icons: lock, email, capture-scrolling, rotate-left/right, flip-h/v, shape types, line styles (solid/dashed/dotted), arrow ends, text align, bold/italic/underline/strikethrough, corner picker, transparent swatch. To be added to `kit/icons/` via `tools/build-icons.py`; page-local glyphs in mockups are replaced then. | kit |

## Waiting for the maintainer

| # | Question | Options | Recommendation |
|---|---|---|---|
| Q1 | Capture hotkeys collide with GNOME's own screenshot keys (`Print`, `Shift+Print`, `Alt+Print`). | (a) Annota offers at first launch to take over the Print keys (GNOME's bindings are moved, reversible) · (b) own keys `Super+Shift+R/W/S`, Print stays GNOME | (a) — a screenshot tool should own Print |
| Q2 | Smart redaction entry | (a) runs automatically when the Redact tool is first opened on a capture · (b) only via button "Find Sensitive Data" | (b) — predictable, no surprise objects |
| Q3 | Retention (auto-trash old captures) | (a) one confirmation the first time it is enabled · (b) only a startup toast with "Review" | (a) |
| Q4 | Light/dark override inside Annota | (a) follow the system only (as specified) · (b) add System/Light/Dark switch | (a) |
| Q5 | Combine captures from the library multi-select bar too (spec says tray only) | yes / no | yes |
