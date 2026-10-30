# add-screen-capture — mockups

Change: `openspec/changes/add-screen-capture/`. Source HTML in `src/`, renders in `png/` (`-light@1x`, `-dark@1x`, `@2x`).
Overlays are 1920×1080 (full-screen, per monitor); editor screens 1280×800. The frozen desktop behind overlays is
`src/assets/desk-a-*.png`, composited only from kit sample content. Captured content stays light in both themes;
only Annota chrome follows the style.

| # | Screen | File (light@1x) |
|---|---|---|
| 01 | Capture launcher popover | [png/01-capture-launcher-light@1x.png](png/01-capture-launcher-light@1x.png) |
| 02 | Region overlay – dragging (size, loupe, colour) | [png/02-region-drag-light@1x.png](png/02-region-drag-light@1x.png) |
| 03 | Region overlay – adjust, fixed size W×H, toolbar | [png/03-region-adjust-fixed-size-light@1x.png](png/03-region-adjust-fixed-size-light@1x.png) |
| 04 | Region overlay – snap to window edge | [png/04-region-snap-window-edge-light@1x.png](png/04-region-snap-window-edge-light@1x.png) |
| 05 | Window mode – GNOME portal picker (Wayland, system UI) | [png/05-window-wayland-portal-picker-light@1x.png](png/05-window-wayland-portal-picker-light@1x.png) |
| 06 | Window mode – X11 click-to-pick | [png/06-window-x11-hover-light@1x.png](png/06-window-x11-hover-light@1x.png) |
| 07 | Delay countdown | [png/07-delay-countdown-light@1x.png](png/07-delay-countdown-light@1x.png) |
| 08 | Permission denied alert | [png/08-permission-denied-light@1x.png](png/08-permission-denied-light@1x.png) |
| 09 | Preferences › Capture – no GlobalShortcuts portal (banner) | [png/09-prefs-shortcuts-fallback-light@1x.png](png/09-prefs-shortcuts-fallback-light@1x.png) |
| 10 | Preferences › Capture – GNOME shortcuts created | [png/10-prefs-shortcuts-created-light@1x.png](png/10-prefs-shortcuts-created-light@1x.png) |
| 11 | Set Shortcut dialog | [png/11-shortcut-capture-dialog-light@1x.png](png/11-shortcut-capture-dialog-light@1x.png) |
| 12 | Region result opened in editor | [png/12-region-result-in-editor-light@1x.png](png/12-region-result-in-editor-light@1x.png) |
| 13 | Paste image from clipboard → new editor tab | [png/13-paste-from-clipboard-light@1x.png](png/13-paste-from-clipboard-light@1x.png) |
| 14 | Drop image file onto Annota | [png/14-drop-image-file-light@1x.png](png/14-drop-image-file-light@1x.png) |

## Scenario → mockup

| Requirement / Scenario | Mockups | Notes |
|---|---|---|
| Region capture / Select and confirm | 02, 03, 12 | Drag shows live `W × H` chip; Enter confirms; editor opens with exactly 400 × 300 (12). |
| Region capture / Cancel | 02, 03 (hint bar, Cancel button) | No UI after Esc: overlay closes, nothing opens — *no further UI, verified by test*. |
| Region capture / Fixed size | 03 | Typing into W / H sets the selection immediately; focus ring on the edited field. |
| Region capture (loupe, snap 8 px, arrow nudge) | 02, 04, 03 | Loupe + pixel colour + coordinates; violet snap guide; hint bar lists ←↑→↓ / Shift ×10. |
| Window capture / Window on Wayland | 05 → 12 | Picker is GNOME Shell UI via portal (`interactive=true`); Annota draws nothing. |
| Window capture (X11 click) | 06 | Hover highlight + window title chip; Tab cycles windows. |
| Full-screen capture / Multi-monitor | 01 (Screen mode) | Monitor under the pointer; `capture-all-monitors` switch in 09/10. Choice of monitor itself has *no UI — verified by test*. |
| Delayed capture / Countdown | 01 (Delay), 07 | Countdown OSD is hidden before the grab; "not in capture" is *verified by test* (pixel check). |
| Global capture hotkey / Hotkey on GNOME 46 | 09 → 10, 11 | Banner + "Create GNOME Shortcuts" creates keybindings running `annota --capture <mode>`; toast confirms. |
| Cursor inclusion / Include cursor | 01, 03 (pointer toggle), 09 | Default off. Pointer presence in image *verified by test*. |
| Image from clipboard / Paste | 01 (menu item), 13 | Shift+Ctrl+V; new tab becomes current in tray "Open". |
| Image from file (open / drop) | 01 (Open Image… Ctrl+O), 14 | Drop zone covers canvas only. |
| Portal permission handling / Permission denied | 08 | Alert dialog, "Open Settings" is the suggested, focused action. |

## Redlines

**Launcher popover (01)** — `AdwPopover` from the header-bar capture button, width 356 px, padding 12.
- Mode toggle group (3 toggles, height 76): icon 24 px (`capture-region`, `capture-window`, `capture-screen`), label 13 px bold,
  shortcut caption 11 px dim: `Shift+Print`, `Super+Shift+W`, `Super+Shift+S` (window/screen = example defaults, configurable).
- Rows 44 px: "Delay" + subtitle "Countdown before capturing" with spin `0 s` (0–60, step 1); "Include pointer" switch (default off).
- Suggested button full width 38 px: "Capture Region" + dim accel. Label follows the mode ("Capture Window", "Capture Screen").
- Separator, menu items: "Paste Image From Clipboard" `Shift+Ctrl+V` (icon `copy`), "Open Image…" `Ctrl+O` (icon `image`).

**Region overlay (02–04)** — undecorated full-screen window per monitor showing the frozen image.
- Outside dim: `rgb(0 0 6 / 48%)` (32 % while dragging). Selection border 1 px white + 1 px outer `rgb(0 0 6/40%)`.
- Handles (after release only): 8, Ø 12 px, white fill, 2 px accent border.
- Size chip above the top-left corner, 6 px gap: `osd-bg-solid`, 13 px bold tabular, "800 × 600". After release it turns accent and adds dim "at x, y".
- Loupe: Ø 136 px, 8× zoom (17 × 17 px), pixel grid, coral crosshair 2 px with 8 px open centre box; placed 26 px right/below the pointer, flips to stay on screen.
  Under it a chip: colour square 14 px + `#RRGGBB` + dim `x, y`.
- Snap: when an edge is ≤ 8 px from a window edge it jumps; 2 px violet guide (`--annota-violet`) along that window edge, dashed window outline, chip "Snapped to window edge" (violet, check icon). Alt suspends snapping.
- Toolbar (03), 14 px below the selection, radius 18, OSD: mode toggles (icon-only 34 px) | `W [800] × H [600] px` entries 74 px | pointer toggle (`cursor`) | delay dropdown (`timer`, Off/1/2/3/5/10 s/Custom) | "Cancel", suggested "Capture" (`camera`).
- Hint bar: pill centred 36 px above bottom edge, 13 px, keycaps 22 px. Text: "Enter Capture · Esc Cancel · ←↑→↓ Nudge 1 px · Shift ×10 · Space Move selection". While dragging: "Drag to select · Shift Square · Alt No snapping · Esc Cancel".

**Window pick X11 (06)** — whole screen dimmed 40 %, hovered window undimmed with 3 px accent + 3 px `accent/35%` ring; accent chip with `capture-window` icon, window title, size. Hint: "Click a window to capture it · Tab Next window · Enter Capture · Esc Cancel".

**Countdown (07)** — centred OSD, radius 30, padding 28/36; ring Ø 148, 8 px track white/18 %, progress Coral; number 72 px heavy; label "Capturing region in 3 s"; keys "Esc Cancel · Enter Capture now". Removed one frame before the grab.

**Permission alert (08)** — `AdwAlertDialog` 420 px; `warning` icon 32 px (warning colour); heading "Screen Access Denied"; body as drawn; caption "Nothing was captured."; buttons "Close", "Open Settings" (suggested, default). Opens `gnome-control-center applications io.github.trosinde.Annota`.

**Preferences › Capture (09/10/11)** — `AdwPreferencesDialog` 660 × 720, view switcher General / Capture / Library.
- Group "Global Shortcuts", rows: Region capture `Shift+Print`, Window capture `Super+Shift+W`, Screen capture `Super+Shift+S` (keycaps + chevron, row opens 11).
- Without portal: warning `AdwBanner` "Global shortcuts are not active yet" + button "Create GNOME Shortcuts"; description "This system has no global-shortcuts portal. Annota adds GNOME keyboard shortcuts instead."; row subtitles "Not active yet".
- After creation: banner gone, subtitles "GNOME shortcut · runs `annota --capture region|window|screen`" (monospace), toast "3 GNOME shortcuts created" + "Open Keyboard Settings".
- With the portal (GNOME ≥ 48) no banner; subtitle "System shortcut" — same layout.
- Group "Capture": Include pointer (off), Default delay spin `0 s`, Capture all monitors (off, subtitle as drawn), After capture dropdown (Open in Editor / Copy to Clipboard / Both).
- Set Shortcut dialog (11) 440 px: Cancel / "Set Shortcut" / Set (suggested); prompt "Press the new shortcut for **Window capture**"; keyboard art 160 × 96; live keycaps 34 px; caption "Esc cancels · Backspace disables the shortcut"; status line success "Not used by GNOME or another app" or error "Used by <name>" (see add-capture-presets 04 for conflict styling).

**Result (12, 13)** — new document becomes current tab in tray "Open" (first position, accent ring), title `Capture YYYY-MM-DD HH-MM-SS` / `Pasted Image`, subtitle `W × H · Not saved`, Select tool active, zoom 100 % (or Fit when larger). Sidebar group "Capture": Size, Source (Region · scale 1× | Window | Screen | Clipboard | File), Captured timestamp. Toast "Region captured · 400 × 300" + "Copy"; paste toast "New capture from clipboard" + "Undo".

**Drop (14)** — canvas area covered by `canvas-bg` 90 % + 2 px dashed accent, radius 12, inset 6 px; 72 px accent circle with `image` icon; title-4 "Drop to Open as New Capture"; caption "PNG, JPG or WebP · drop onto an annotation tool to insert instead".

## Open questions
- Wayland window capture: portal picker look is GNOME's; if the portal returns a full screen instead of a window (older portals), do we fall back to the X11-style pick overlay on the frozen image? Mockup 06 would cover it.
- Overlay toolbar delay dropdown values (1/2/3/5/10 s + Custom) are proposed, not in spec.
