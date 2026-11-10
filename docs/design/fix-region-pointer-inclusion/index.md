# fix-region-pointer-inclusion — mockups

Change: `openspec/changes/fix-region-pointer-inclusion/`. Source HTML in `src/`, renders in `png/`
(`-light@1x`, `-dark@1x`, `@2x`). Overlays are 1920×1080 (full-screen, per monitor); the redline sheet is
1280×1560. The frozen desktop is the same asset as add-screen-capture
(`../add-screen-capture/src/assets/desk-a-*.png`, referenced, not copied) so both changes show the same
product. Captured content stays light in both themes; only Annota chrome follows the style — and the
region overlay is OSD chrome, so light and dark renders differ only in the redline sheet.

This change adds **one** element to the existing region overlay (selection, handles, size chip, loupe,
snap guide, hint bar, toolbar): the **pointer preview**. It consists of three parts that reuse existing
anchors instead of adding a new floating surface:

1. a **preview arrow** at the pointer hot spot — the same arrow `frame::pointer_path` composites,
2. a **pointer chip** docked 30 px right of the hot spot, in the 26 px band *above* the loupe,
3. a **segment prepended to the hint bar**, which is where the working gesture is taught.

| # | Screen | File (light@1x) |
|---|---|---|
| 01 | Pointer will be included — selection settled, pointer moved inside | [png/01-pointer-included-light@1x.png](png/01-pointer-included-light@1x.png) |
| 02 | Pointer will not be included — pointer on another window | [png/02-pointer-not-included-light@1x.png](png/02-pointer-not-included-light@1x.png) |
| 03 | Indication follows the selection — selection nudged out from under the pointer | [png/03-indication-follows-selection-light@1x.png](png/03-indication-follows-selection-light@1x.png) |
| 04 | No indication when excluded — same scene as 01, Include Pointer off | [png/04-pointer-excluded-no-indication-light@1x.png](png/04-pointer-excluded-no-indication-light@1x.png) |
| 05 | Dragging — pointer rides the edge (preview **and** loupe) | [png/05-drag-pointer-on-edge-light@1x.png](png/05-drag-pointer-on-edge-light@1x.png) |
| 06 | Released on the edge — the trap, and how the overlay names it | [png/06-released-on-edge-light@1x.png](png/06-released-on-edge-light@1x.png) |
| 07 | Preview states and redlines (sheet) | [png/07-preview-states-redlines-light@1x.png](png/07-preview-states-redlines-light@1x.png) |

01, 03 and 04 are deliberately the **same scene with the same pointer position** (310, 488 — on the
Postcode field with the validation error). Only the selection rectangle (01 → 03) or the Include Pointer
toggle (01 → 04) differs, so the three PNGs read as one before/after sequence.
05 → 06 is the same drag one frame apart: still dragging, then released.

## Scenario → mockup

### ADDED — Requirement: Pointer preview in the region overlay

| Scenario | Mockups | Notes |
|---|---|---|
| Pointer will be included | **01**, 07 §1–§3 | Solid preview arrow at the hot spot inside the selection, chip “Pointer in capture” with the coral keyline, hint bar segment “Pointer in capture”. Cursor is the overlay’s own `move` cursor (phase *Adjusting*, pointer inside the selection), drawn over the preview. |
| Pointer will not be included | **02**, also 05, 06 | Ghosted, slashed preview arrow in the dimmed area, chip “Pointer not in capture”, hint bar teaches the gesture. 02 is the plain “pointer is somewhere else” case. |
| Indication follows the selection | **03** (compare with 01) | Same pointer, selection moved to x = 380 with `Shift+→`; the preview flipped to “not in capture” with no confirmation. Mid-gesture: keyboard nudge keeps phase *Adjusting*, so the toolbar stays and the loupe stays hidden (`overlay.rs:556`). |
| No indication when excluded | **04** (compare with 01) | Include Pointer off: no preview arrow, no chip, hint bar is exactly the one from add-screen-capture 03. Only the toolbar toggle differs from 01. |

### MODIFIED — Requirement: Cursor inclusion

| Scenario | Mockups | Notes |
|---|---|---|
| Include cursor (screen capture) | no screen | Screen mode is untouched; pointer presence in the image is *verified by test*. |
| Pointer inside the selection | **01** | What the user sees immediately before pressing `Enter`; the arrow is drawn where it will land in the image. |
| Pointer outside the selection | **02**, **03** | No pointer is drawn; the image is otherwise exactly the selected region — *verified by test*. |
| Pointer on the selection edge | **05** (before release), **06** (after release) | 06 is the trap: the released drag leaves the hot spot on the lower-right corner, so nothing is drawn. The chip adds the dim suffix “on the edge” and the hint bar names the fix. |
| Delayed region capture places the pointer | no new screen; add-screen-capture 07 | The countdown OSD is unchanged. During the countdown no overlay is on screen, so there is no preview to draw. |
| Cursor excluded | **04** | No indication at all. |

### Not drawn on purpose

- **Pointer over the overlay toolbar or hint bar.** Preview arrow and chip are hidden there (they would
  sit on top of Annota’s own chrome and the result is never “inside” anyway); the hint-bar segment keeps
  showing “Move the pointer into the selection to include it”. Behaviour only, no new screen.
- **`state.pointer == None`** (confirmed from the keyboard without the pointer ever entering the overlay,
  design.md Risks): treated exactly as “outside” — no preview arrow, no chip, hint bar shows the
  “outside” segment. Same pixels as 02 minus the arrow and chip.
- **Pointer on another monitor than the selection**: the existing monitor filter applies, so that overlay
  shows no preview; the overlay on the pointer’s monitor shows the “outside” state. Same pixels as above.

## Redlines

### Preview arrow (new)

- Drawn **only** while Include Pointer is on, the overlay has a pointer position on this monitor, and the
  pointer is over the frozen image (not over the toolbar or the hint bar).
- Identical outline to `frame::pointer_path`: **22 px tall, 15 px wide**, hot spot at the tip, body
  extending right and down. Do not re-draw it — reuse the same path so preview and result cannot diverge.
- **In capture** — fill `#ffffff`, 1 px `#000006` outline on a 2.8 px `#000006` halo (same path stroked
  twice), drop shadow `0 1px 2px rgb(0 0 6 / 45 %)`.
- **Not in capture** — fill `rgb(255 255 255 / 48 %)` over a `rgb(0 0 6 / 45 %)` fill, 1.5 px `#ffffff`
  outline on a 3 px `rgb(0 0 6 / 60 %)` halo, plus a slash from the lower left to the upper right of the
  glyph box: 2.1 px `#ffffff` on a 4.4 px `#000006` halo at 75 %. The slash stays inside the glyph box so
  it does not merge with the overlay’s crosshair cursor.
- Z-order: **below** the system cursor, above the dim and the selection border. The state is never carried
  by colour alone — solid vs. ghosted-and-slashed.
- Painted in the same `draw` pass as the loupe (`overlay.rs` canvas draw func), driven by the **same**
  `contains` test as `region()` (design.md D2/D3), re-evaluated on every pointer motion and every
  selection change.

### Pointer chip (new)

- Anchored **30 px right of the hot spot**, vertically centred on it; flips to 30 px left of the hot spot
  when it would leave the monitor. It therefore always sits in the 26 px band above the loupe and can
  never overlap it (see 05 and 07 §5).
- Body identical to the existing loupe chip: `--osd-bg-solid` `#303034`, radius 6 px, padding
  `4 / 10 / 4 / 12`, 13 px bold, shadow `0 1px 4px rgb(0 0 6 / 35 %)`, glyph 14 px, gap 6 px.
- Leading 3 px inset keyline: `--annota-coral` `#ff5a4e` when the pointer is in, `rgb(255 255 255 / 60 %)`
  when it is not. The glyph takes the same colour.
- Optional dim suffix, 13 px regular at 70 % opacity, same as the loupe chip’s coordinates.

### Hint bar (extended, never replaced)

- The existing key list (`hint_drag`, `hint_snap`, `hint_adjust` in `data/ui/region-overlay.blp`) does not
  change. While Include Pointer is on, **one segment plus a `·` separator is prepended** to whichever hint
  box is visible. While Include Pointer is off, nothing is prepended (04).
- Segment: 14 px glyph + 13 px label, 6 px gap, same `--osd-fg`. Glyph coral in the “in” state.

### Strings

| Element | English | German |
|---|---|---|
| Chip, pointer inside | Pointer in capture | Mauszeiger wird aufgenommen |
| Chip, pointer outside | Pointer not in capture | Mauszeiger wird nicht aufgenommen |
| Chip suffix, hot spot within 1 px of the border | on the edge | auf dem Rand |
| Hint bar segment, pointer inside | Pointer in capture | Mauszeiger wird aufgenommen |
| Hint bar segment, pointer outside or on the edge | Move the pointer into the selection to include it | Mauszeiger in die Auswahl bewegen, damit er aufgenommen wird |
| Toolbar toggle tooltip (unchanged) | Include Pointer | Mauszeiger einschließen |

No other string changes. No toast, no banner, no dialog: a dropped pointer is stated **before**
confirmation, not reported afterwards.

### Unchanged, shown for reference

- Selection border 1 px `#ffffff` + 1 px `rgb(0 0 6 / 40 %)`; outside dim 48 %, 32 % while dragging.
- Handles Ø 12 px, white fill, 2 px accent border, only after release.
- Size chip 6 px above the top-left corner; accent (`--accent-bg`) once the selection is settled, with the
  dim `at x, y` suffix.
- Loupe Ø 136 px at +26 / +26 px from the hot spot, colour chip 10 px below it. Visible in phases
  *Idle*, *Dragging*, *Resizing* only — that is why 01, 03, 04 and 06 show no loupe and 05 does.
- Overlay toolbar 14 px below the selection, left-aligned to it: mode toggles | `W … × H … px` |
  **Include Pointer** (`cursor`, 34 × 34) | delay dropdown | Cancel | Capture. Position unchanged.
- Hint bar pill centred, 36 px above the bottom edge, 13 px, keycaps 22 px.

### Kit additions needed

Two symbolic 16 × 16 glyphs, drawn page-locally in these mockups and to be added to `kit/icons/` via
`tools/build-icons.py` (decision D8 covers this pattern):

| Id | Use |
|---|---|
| `pointer-included` | filled pointer silhouette — chip and hint bar, “in capture” state |
| `pointer-excluded` | outlined pointer with a slash — chip and hint bar, “not in capture” state |

## Open questions

- **The toolbar’s Capture button can never include the pointer.** Clicking it puts the hot spot on the
  button, i.e. outside the selection, so a mouse-driven capture is always pointer-free; only `Enter`
  (or a delay) can include it. The mockups teach `Enter` in the hint bar and leave the button alone. If
  the maintainer wants this spelled out, the Capture button’s tooltip could become “Capture (Enter) —
  the pointer is only included when it is inside the selection”. Not drawn.
- **Screen 03 stages “follows the selection” as a keyboard nudge**, because that is the only way the
  selection can move while the mouse stays still (`nudge` → `move_by`, `overlay.rs:449`). A mouse resize
  always puts the hot spot on the handle, i.e. on the edge, which is screen 06’s case. If the overlay
  later gains mouse-free resize handles this screen should be revisited.
- The preview arrow and the system cursor necessarily share the hot spot (05, 06 show the real pile-up at
  a corner: handle + resize cursor + preview). Hiding the system cursor while the preview is on would be
  cleaner (“what you see is what you capture”) but costs the crosshair’s precision and the handle
  affordances, so it is **not** proposed here. Flagged in case the live test finds the pile-up unreadable.
