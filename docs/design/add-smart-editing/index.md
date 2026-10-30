# add-smart-editing — mockups

Window 1280 × 800, editor shell from `docs/design/kit/template.html`. Every screen has
`png/<name>-{light,dark}@{1,2}x.png`; sources in `src/*.html`. The capture in every screen is
the kit sample `a-harbor-backup` (a settings dialog). `assets/harbor-autofill.png` is that
sample with the "Start backup" button removed and its area filled from the background.

## Scenario → mockup

| Requirement · Scenario | Mockup | Notes |
|---|---|---|
| Entry points | `10-main-menu-entry` | Main menu section "Smart Editing": Smart Move (Ctrl+Shift+M), Simplify… (Ctrl+Shift+Y). |
| Smart move · analysing | `01-smart-move-analysing` | Canvas veiled 35 % white, OSD "Detecting elements…" + Cancel; sidebar row "Analysing image… · Runs on this computer". |
| Smart move · detected elements | `02-smart-move-detected` | Detail 55: UI elements outlined azure dashed, text blocks violet dashed; hovered button gets a solid outline + tooltip "Button · drag to move". Legend in sidebar. |
| Smart move · detail slider | `03-smart-move-low-detail` | Detail 15: only large blocks (sidebar, three cards, headings, button row); "9 elements · 0 text blocks". |
| Smart move · Move button (dragging) | `04-smart-move-dragging` | "Start backup" dragged left with a lifted shadow; its old place already shows background; dotted outline marks the origin; position badge x 700 · y 798. |
| Smart move · Move button (result) | `05-smart-move-result` | Button at its new place with handles; toast "Moved "Start backup" · background filled" + Undo. |
| Simplify · preview | `06-simplify-before-after` | Before / After side by side while the dialog is open; detail 60, palette Annota Neutral, Show Original switch; Cancel / Apply footer. |
| Simplify · Simplify screenshot (applied) | `07-simplify-applied-coral` | Coral palette applied: text lines → rounded bars, controls → flat shapes, layout unchanged. One bar selected (editable object). Toast "Simplified · 46 shapes" + Undo; footer "Revert to Original". |
| Simplify · detail + palette | `08-simplify-low-detail-ink` | Detail 20 + "Ink on Paper": only headings and primary control remain; outline style. |
| Simplify · custom palette | `09-simplify-new-palette` | "New Palette…" dialog: name + five colour roles (Headings, Text Lines, Controls, Accent, Background). |

## Redlines

**Smart Move sidebar.** Header: select icon + "Smart Move" + switch (on). Detail slider 0–100
(default 55, marks at 0/50/100) with captions "Large blocks" / "Single controls". "Found" row:
"<n> elements · <m> text blocks". Legend: UI element (azure dashed), Text block (violet dashed).
When an element is selected: group "Selected Element" (type, size, origin) + Delete, Duplicate.
Hint: "Drag an element to move it. Its old place is filled from the surrounding background.
Delete removes it."

**Detection overlay.** Outlines 2 px, dash 6/4, radius 3, padded 4 px around the element (at
100 %). UI element = `--annota-azure`, text block = `--annota-violet`. Hover: solid 3 px +
10 % azure fill + tooltip "<Type> · drag to move". Dragging: element lifts with a drop shadow
(0 6 10, 35 %), origin shown as 1.5 px azure dashed 2/3 outline, live position badge in
accent below the element. Background fill happens live during the drag.

**Analysing.** Runs in a worker thread; canvas veiled `rgb(255 255 255 / 35%)`, centred OSD with
spinner, label "Detecting elements…", Cancel. Toggling smart move off removes all overlays;
moved elements stay where they are.

**Simplify sidebar.** Header shape icon + "Simplify". Detail 0–100 (default 60) with hint.
Palette list (radio rows 50 px, four colour chips: heading, text, controls, accent):
Annota Neutral (default), Coral, Azure, Ink on Paper, then "New Palette…" button row.
"Show Original" switch while previewing. Footer: Cancel / Apply (preview) → "Revert to
Original" (applied).

**Simplified shapes.** Text line → rounded bar, height 70 % of the line height, full rounding;
heading bars use the heading colour, body lines the text colour. Controls → flat rounded
rectangles (radius 8 at 100 %), switch → pill + white knob, primary button → accent fill.
Cards keep their outline (2 px, palette line colour) and radius 12. Every shape is a normal
annotation object (select, move, recolour, delete). Detail < 40 drops secondary text lines,
subtitles and row icons.

**Palette dialog.** 460 px, rows with colour chips (hex shown) for Headings, Text Lines, Controls,
Accent, Background. Stored locally.

## Open questions

1. Smart Move as a switch in the Select tool's sidebar vs. a separate tool: the mockups make it
   a mode of Select (V) reachable from the main menu / Ctrl+Shift+M, since the shared tool
   list has no Smart Move key. Confirm.
2. Detection overlay colours (azure / violet) use annotation tokens; an engineer might prefer
   a UI-accent colour. Kept distinct from the blue selection handles on purpose.
3. Simplify applied replaces the image with shapes — "Revert to Original" restores the capture
   in one undoable step. Confirm that Simplify is not offered as a non-destructive effect in
   the effects stack.
