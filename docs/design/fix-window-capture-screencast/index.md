# fix-window-capture-screencast — mockups

Change: `openspec/changes/fix-window-capture-screencast/`. Source HTML in `src/`, renders in `png/`
(`-light@1x`, `-dark@1x`, `@2x`).

- **01** is a flow sheet, 3192 × 1416. Panels a, c, d and e are scaled previews: Annota windows
  1280 × 800 at 45 %, a 1920 × 1200 monitor at 30 %. Panel b is a 1 : 1 crop. The cards under the
  panels are 1 : 1 and are the binding detail.
- **02** is the waiting OSD, 1920 × 1608. The top 1920 × 1080 is a 1 : 1 screen, placed like
  add-screen-capture 07. Below it are the states and redlines.

Annota adds one surface of its own: the waiting OSD (design.md D8). All other Annota surfaces
exist already. The chooser (01c, 02) is a schematic placeholder for GNOME's dialog, not a drawing
of it.

These sheets replace [add-screen-capture 05](../add-screen-capture/png/05-window-wayland-portal-picker-light@1x.png)
as the reference for window mode. Mockup 05 remains valid only for the fallback ("No window source").

| # | Screen | File (light@1x) |
|---|---|---|
| 01 | Window capture flow: a welcome page, Window card activated → b waiting OSD → c desktop window chooser (placeholder) next to the OSD → d countdown (only with a delay) → e editor with the window image and toast | [png/01-window-capture-flow-light@1x.png](png/01-window-capture-flow-light@1x.png) |
| 02 | Waiting OSD "Waiting for a window…" with Cancel, next to the placeholder chooser (its child). States: default, Cancel hover, Cancel keyboard focus. Redlines. | [png/02-window-wait-osd-light@1x.png](png/02-window-wait-osd-light@1x.png) |

Dark: `png/01-window-capture-flow-dark@1x.png` and `png/02-window-wait-osd-dark@1x.png`. 2×: the
same names with `@2x`.

## Scenario → mockup / test / live check

Panels are named `01a`–`01e`. The six cards along the bottom of 01 are `01 branch: <title>`.
Tests and live checks refer to `tasks.md` of this change.

| Spec / Requirement / Scenario | Mockup | Evidence | Notes |
|---|---|---|---|
| capture / Window capture / Window on Wayland | 01a → 01b → 01c → 01e | Live check 6.1 (row 1, scale 1.25 and 1, maximised and small window). Crop: unit tests 4.2. Session closed: drop-order test 4.1. | Chooser is GNOME's; image edge = window edge (01e detail). |
| capture / Window capture / Window mode from the welcome page | 01a → 01b/01c → 01e | Live check 6.1 (rows 1 and 2). | Toast "Window captured · W × H" with Copy, string already in `delivery.rs`. GNOME's screenshot UI never appears. |
| capture / Window capture / Waiting for the chooser | **02**, 01b, 01c | Live check 6.1 (row 2, from a GNOME keybinding with no Annota window open). | The OSD appears together with the chooser and is its parent. It goes once a window is chosen. |
| capture / Window capture / Cancel while waiting | **02** (Cancel states), 01b detail, 01 branch: Cancel while waiting | Test: Cancel/`Esc` closes the session → `Cancelled` (task 4.4), then a second capture starts (`BUSY` cleared). Live check 6.1 (chooser never shown, Cancel). | No message. The OSD goes, the session closes. |
| capture / Window capture / Chooser closed | 01 branch: Chooser closed, 01b detail | Test 4.4 (`live_tests.rs`, `Start` answered with code 1 → `Cancelled`), live check 6.1 (row 3). | Once GNOME reports it, the waiting OSD disappears and nothing is shown. If GNOME never reports it, the OSD stays until Cancel (D6). |
| capture / Window capture / Delayed window capture | 01c → 01d → 01e, skip arc for no delay | Test 4.3 (`window_steps(delay)`), live check 6.1 (row 4). | Countdown OSD of add-screen-capture 07, label "Capturing window in 3 s". |
| capture / Window capture / Countdown cancelled | 01d, 01 branch: Countdown cancelled | Test 4.1 (drop guard closes the session), live check 6.1 (row 5: indicator disappears). | `Esc` on the countdown OSD. No message. |
| capture / Window capture / No frame arrives | 01 branch: No frame in 5 s | Test 4.4 (frame timeout and no opaque pixel → `Failed`), unit test 4.2 (no opaque pixel). | Existing toast "The capture failed. Nothing was captured." |
| capture / Window capture / No window source | 01 branch: No window source; add-screen-capture 05 | Test 4.3 (fallback branch), measurement 2.4. | Unchanged Screenshot-portal UI, no waiting OSD. Ending without image → "Nothing was captured". |
| capture / Cursor inclusion / Include cursor | no screen (as fix-region-pointer-inclusion) | Existing screen-capture tests. | Screen mode unchanged by this change. |
| capture / Cursor inclusion / Pointer inside the selection | fix-region-pointer-inclusion 01 | That change's tests. | Text carried over from `fix-region-pointer-inclusion`; nothing new here. |
| capture / Cursor inclusion / Pointer outside the selection | fix-region-pointer-inclusion 02, 03 | That change's tests. | As above. |
| capture / Cursor inclusion / Pointer on the selection edge | fix-region-pointer-inclusion 05, 06 | That change's tests. | As above. |
| capture / Cursor inclusion / Delayed region capture places the pointer | no new screen (add-screen-capture 07, per fix-region-pointer-inclusion) | That change's tests. | As above. |
| capture / Cursor inclusion / Pointer in a window capture | 01 branch: Include Pointer | Test 4.5 (`cursor_mode` choice, 4 combinations), measurement 2.3 (stop point D5), live check 6.1 (row 6). | No screen: the pointer is part of GNOME's frame. |
| capture / Cursor inclusion / Cursor excluded | 01 branch: Include Pointer; region/screen: fix-region-pointer-inclusion 04 | Test 4.5 (HIDDEN when off), live check 6.1 (row 6). | |
| debian-packaging / Runtime dependencies / Dependencies resolved | no screen | Task 3.2, `lintian -I --pedantic` in 7.4, install with apt. | |
| debian-packaging / Runtime dependencies / Window capture works on a fresh install | no screen | Live check 6.2 (fresh Ubuntu 24.04, apt install, window capture delivers an image). | |

## Redlines

### Strings
| Where | en | de | Status |
|---|---|---|---|
| Waiting OSD label | `Waiting for a window…` | `Warte auf ein Fenster …` | **new** |
| Waiting OSD button | `Cancel` | `Abbrechen` | existing msgid |
| Waiting OSD hint | keycap `Esc` + `Cancel` | `Abbrechen` | existing msgid, as 07 |
| Countdown | `Capturing window in {seconds} s` | existing | `countdown.rs` |
| Success toast | `Window captured · {width} × {height}` + **Copy** | existing | `delivery.rs` |
| Failure toast | `The capture failed. Nothing was captured.` | existing | no button |
| Fallback only | `Nothing was captured` | existing | |

German ellipsis convention: a space before `…` (19 of 21 entries in `po/de.po`, e.g. "Öffnen …").
English has no space (`Open…`). Both use the single character `…` (U+2026), not three dots.

### Order (binding)
1. **a** Window card, launcher Window mode, the Window shortcut or `annota --capture window`. Annota
   hides its windows, as for every capture.
2. **b + c** `Start(parent)` with the waiting OSD as parent (D8). The OSD and GNOME's chooser appear
   together. Annota sends `SelectSources { types: WINDOW, multiple: false, cursor_mode: EMBEDDED |
   HIDDEN, persist_mode: 0 }` (D1). No restore token, so the chooser appears every time.
3. The OSD goes when the chooser answers. On Cancel / `Esc` the session closes and nothing is shown.
4. **d** Only with a delay: the countdown starts **after** a window is chosen. With no delay this
   step is skipped (violet skip arc on the sheet).
5. **e** One frame, cut to the window (D3), session closed, then delivery with `Source::Window`.
   The session is closed before the editor is shown.

### Waiting OSD (02, 01b)
- **Body:** same as the countdown OSD of 07: `--osd-bg`, backdrop blur 16 px, radius 30,
  `--shadow-osd`. Fixed width 300 px. Padding 30 top, 36 sides, 24 bottom. Gap 18 between items.
- **Activity ring:** 72 × 72, stroke 6. The track is white at 18 %. The arc is `--annota-coral`,
  a quarter turn with round caps. It turns once per 1.2 s. With GNOME animations off it stands
  still. The kit icon `capture-window` sits in the centre, 32 px, white. The ring echoes the
  countdown ring but carries no number, so the two OSDs are told apart by more than colour.
- **Label:** 15 px bold, white.
- **Cancel:** pill, 34 px high, min 120 px wide, `--osd-fill-hover`. Hover and pressed:
  `--osd-fill-active`. Keyboard focus: 2 px ring `rgb(120 174 237 / 80%)`, offset 1, as the OSD
  entries in 03. Cancel has default focus. `Enter` / `Space` activate it. `Esc` cancels anywhere in
  the OSD.
- **Hint:** keycap `Esc` + "Cancel", 13 px at 85 % opacity. Same as 07.
- **Window:** its own top-level, non-modal. Nothing is dimmed. It is centred on the monitor under
  the pointer, like `countdown.blp`. Its exported handle is `Start`'s parent. It does not appear in
  the chooser's list. GNOME places the chooser: 02 draws it beside the OSD, but it may cover it.
- **Second window-capture request:** raises the OSD and starts nothing new.
- **Accessibility:** role alert, so the label is read once when it appears. The Cancel target is at
  least 120 × 34.

### Panel e — editor
- Header bar per decision D1: New Capture · Undo/Redo (insensitive) | title
  `Capture 2026-11-14 10-42-17`, subtitle `1280 × 800 · Not saved` | Show Properties (F9, active) ·
  Copy · Export split button · main menu · close.
- Canvas at the fit zoom (62 % for a 1280 × 800 image in the 928 × 621 canvas). The thin shadow around
  the image is the canvas's own page shadow, the same for every capture. The image itself carries no
  shadow and no transparent margin: its edge is the window's edge.
- Transparent pixels inside the window (rounded top corners, 12 px at 1×) stay transparent and show
  the canvas checkerboard (detail at 4 ×). Only the outer bounding box is cut (D3).
- Sidebar group "Capture": Size `1280 × 800 px`, Source `Window · scale 1×`, Captured
  `14 Nov 2026, 10:42:17`. Same rows as add-screen-capture 12.
- Toast: centred over the canvas column (between tool strip and sidebar), 24 px above the tray,
  as fixed in add-export-share (`b2a151c`).

### Chooser placeholder (01c, 02)
- Never implement or imitate it. It stands for GNOME's dialog, whose look and wording vary by
  version. The dashed frame, grey bars and unlabeled buttons are deliberate.
- GNOME's screen-sharing indicator is shown as a dashed "indicator" pill in the top bar (panel d).
  It is GNOME's, not drawn by Annota. It is visible from `Start` until the session closes, including
  the whole countdown (accepted, design.md D4).

### Branches with no screen of their own
| Branch | What the user sees |
|---|---|
| Cancel while waiting (Cancel or `Esc`) | The waiting OSD goes. The session closes, which also ends GNOME's pending request. No image, no message. The next capture starts normally. |
| Chooser closed | Once GNOME reports it, the waiting OSD goes. No image, no window, no toast. Windows that were open come back as they were. |
| No stream, or no frame within 5 s | Failure toast in the window Annota presents for messages (the welcome page if none was open). Cause on stderr. |
| Countdown cancelled (`Esc`) | Nothing. The session closes, GNOME's indicator disappears. |
| No window source | Screenshot-portal UI as add-screen-capture 05, unchanged, no waiting OSD. |
| Include Pointer on / off | No UI change. The setting only chooses `cursor_mode`. |

## Open questions
- Whether "Abbrechen" in GNOME's chooser reliably answers `1` is left to the live checks (D6). If it
  does not, the waiting OSD stays until the user cancels it. That is honest, but it is a second click.
- Whether `EMBEDDED` really puts the pointer into a window frame on GNOME 50 is not yet measured (task
  2.3, stop point D5).
- If GNOME stacks the chooser exactly over its parent, the OSD is hidden while the chooser shows. It
  becomes visible only when the chooser fails to show, which is the case it exists for. To be
  confirmed in the live check.
