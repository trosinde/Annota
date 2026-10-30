# add-capture-presets — mockups

Change: `openspec/changes/add-capture-presets/`. Builds on `add-screen-capture` (same overlay, launcher and preferences).
Overlays 1920×1080, editor 1280×800. Desktop background `src/assets/desk-a-*.png` (kit sample content only).

| # | Screen | File (light@1x) |
|---|---|---|
| 01 | Preferences › Capture › Presets list | [png/01-presets-list-light@1x.png](png/01-presets-list-light@1x.png) |
| 02 | Edit Preset dialog | [png/02-preset-editor-light@1x.png](png/02-preset-editor-light@1x.png) |
| 03 | New Preset – recording a shortcut | [png/03-preset-shortcut-recording-light@1x.png](png/03-preset-shortcut-recording-light@1x.png) |
| 04 | New Preset – shortcut conflict | [png/04-preset-shortcut-conflict-light@1x.png](png/04-preset-shortcut-conflict-light@1x.png) |
| 05 | Preset run – fixed 800×600 frame | [png/05-preset-fixed-frame-light@1x.png](png/05-preset-fixed-frame-light@1x.png) |
| 06 | Preset result – copied, no editor | [png/06-preset-copied-toast-light@1x.png](png/06-preset-copied-toast-light@1x.png) |
| 07 | Multi-area selection (3 numbered areas) | [png/07-multi-area-selection-light@1x.png](png/07-multi-area-selection-light@1x.png) |
| 08 | Multi-area result in editor | [png/08-multi-area-result-light@1x.png](png/08-multi-area-result-light@1x.png) |
| 09 | Launcher with Presets section | [png/09-launcher-presets-menu-light@1x.png](png/09-launcher-presets-menu-light@1x.png) |
| 10 | Time-lapse setup dialog | [png/10-time-lapse-setup-light@1x.png](png/10-time-lapse-setup-light@1x.png) |
| 11 | Time-lapse running indicator | [png/11-time-lapse-running-light@1x.png](png/11-time-lapse-running-light@1x.png) |
| 12 | Time-lapse finished toast | [png/12-time-lapse-finished-light@1x.png](png/12-time-lapse-finished-light@1x.png) |

## Scenario → mockup

| Requirement / Scenario | Mockups | Notes |
|---|---|---|
| Capture presets (define mode, size, delay, cursor, effect, output, shortcut) | 01, 02, 03, 04, 09 | Presets stored in GSettings JSON; list order = launcher order. |
| Capture presets / Preset hotkey | 05 → 06 | Ctrl+Print shows a fixed 800 × 600 frame; Enter copies; no editor window, only the result toast. |
| Multi-area capture / Two areas | 07 → 08 | Ctrl+drag adds areas; result keeps screen positions on a transparent canvas (checkerboard). |
| Time-lapse capture / Interval | 10 → 11 → 12 | 10 s × 1 min = 6 images. File count itself *verified by test*. |
| Capture info metadata (proposal) | — | Metadata is stored, not shown here; display lives in add-image-effects ("Capture info"). *No UI in this change — verified by test.* |

## Redlines

**Presets group (01)** — in Preferences › Capture below "Global Shortcuts". Header "Presets", description "Saved capture settings. Run them from the capture menu or with their own shortcut."
Row: prefix mode icon (`capture-region`/`capture-window`/`capture-screen`/`timer`), title = preset name, subtitle = summary "Mode · size/delay/effect · output" (13 px dim), suffix keycaps or dim "No shortcut", flat chevron button "Edit Preset". Last row: button row "+ Add Preset".

**Preset dialog (02–04)** — `AdwDialog` 560 × 740 over preferences. Header: Cancel | "Edit Preset"/"New Preset" | Save (suggested; insensitive while a conflict exists).
- Entry row "Name". Group "Capture": Mode toggle group Region / Window / Screen / Time-lapse (with icons); "Fixed size" switch + two 84 px entries `800 × 600` (subtitle "Place the frame, then press Enter"); Delay spin; Include pointer switch.
- Group "After Capture": Quick effect dropdown (None + quick effects from add-image-effects); Output toggle Editor / Clipboard / File (File reveals a folder row, not drawn).
- Shortcut row: keycaps + flat `edit-clear` button "Remove Shortcut". Recording (03): row tinted `accent-a12`, focus ring, subtitle accent "Press keys… Esc cancels, Backspace removes", pending keycap "…".
  Conflict (04): subtitle error "Shift+Print is already used by “Region capture”", keycaps outlined 1 px destructive, button "Replace".
- Destructive text button "Delete Preset" (confirm with alert, standard).

**Preset frame (05)** — fixed-size selection, no resize handles, pointer = grab hand; accent chip "<icon> 800×600 to clipboard · 800 × 600 · fixed". Hint: "Move the frame · Enter Copy to clipboard · ←↑→↓ Nudge · Esc Cancel" (verb = preset output).

**Copied toast (06)** — Annota-owned OSD on the desktop (no editor), bottom-centred 64 px from the bottom, radius 18, `toast-bg`, auto-hides after 5 s: thumbnail 96 × 72, "Copied to clipboard" bold, caption "800 × 600 · preset “…” · saved to library", button "Open in Editor", close. On Wayland, if Annota cannot place a layer surface, fall back to a GNotification with the same text and action.

**Multi-area (07)** — areas are holes in one dim mask; each area: 1 px white border, number badge Ø 28 accent with 2 px white ring at its top-left corner, size chip below. The area being drawn has a dashed border. Top chip (accent): "<selection icon> 3 areas · result W × H on transparent canvas". Hint: "Ctrl + drag Add area · Ctrl + click Remove area · Backspace Remove last · Enter Capture all · Esc Cancel".

**Multi-area result (08)** — canvas = bounding box of all areas, gaps transparent (checker). Sidebar "Capture": Size, Source "Region · 3 areas", Background "Transparent"; hint text as drawn.

**Launcher (09)** — same popover as add-screen-capture 01, plus section "Presets" (menu items with mode icon and accel), then "Time-Lapse…" (`timer`) and "Manage Presets…" (`settings`, opens 01).

**Time-lapse setup (10)** — dialog 520 px: Cancel | "Time-Lapse" | "Select Region…" (suggested). Rows: Interval spin (1 s–60 min, subtitle "1 s – 60 min"), Stop after dropdown (1 min / 5 min / 15 min / 1 h / Until stopped), Folder (default `~/Pictures/Annota/Time-lapse/<timestamp>`), Include pointer. Info line: "6 images will be saved. Stop any time with Shift+Ctrl+Print."

**Running (11)** — region outline 2 px dashed white (not captured), chip "Time-lapse region · W × H". Control pill under the region: Coral dot, "3 of 6", progress 120 px (Coral), dim "next in 7 s", Pause, "Stop" (`stop`). Top-bar indicator: Coral pill "⏱ 3/6". Stop shortcut `Shift+Ctrl+Print`.

**Finished (12)** — editor toast "Time-lapse finished · 6 images saved" + "Open Folder" + close; new images at the front of tray "Recent" with a sequence badge.

## Open questions
- Stop shortcut: the brief gives Shift+Ctrl+Print for "stop recording"; I reused it for stopping a time-lapse. OK to share?
- Copied toast outside the editor (06): needs a small Annota OSD window; acceptable on Wayland, or GNotification only?
- "Stop after" values for time-lapse are proposed (spec only fixes the interval range).
