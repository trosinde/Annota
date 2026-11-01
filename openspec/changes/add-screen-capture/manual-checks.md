# Manual checks — add-screen-capture

Every scenario has automated coverage of its decision logic (`cargo test`, see the last column).
What the tests cannot do is talk to a real xdg-desktop-portal, GNOME Shell or GNOME's keybinding
daemon: this change was developed on the maintainer's own desktop, where a real portal call would
capture the actual screen and real keybindings would change the real session. **None of the rows
marked "live" has been run yet.** Run them on Ubuntu 24.04 (GNOME 46, no GlobalShortcuts portal)
and Ubuntu 26.04 (GNOME 50, with the portal) before archiving, with the installed `.deb` (so
`annota` is on `PATH`) or `meson devenv -C _build ./src/annota`.

What *was* run (2026-10-30): the full UI in an isolated session — private D-Bus daemon without
service activation (so no portal can start), Xvfb, `GSETTINGS_BACKEND=memory` — covering the
launcher popover, Preferences › Capture without portal (banner, "Create GNOME Shortcuts" →
toast and row subtitles, in memory only), the Set Shortcut dialog with a free and a conflicting
key, and `annota --capture …` forwarded to a running instance ending in the "portal not running"
toast. `dconf` was checked before and after: unchanged.

| Scenario | Status | How to check (live) | Automated |
|---|---|---|---|
| Region / Select and confirm | live check pending | Launcher (header-bar button or `Ctrl+N`) → Capture Region. Drag ~400×300, watch the size chip, release, press Enter. With after-capture "Open in Editor" a 400×300 PNG appears in the save folder and a toast says "Region captured · 400 × 300 · Saved as …". | `geometry::drag_400_by_300`, `frame::crop_has_exact_size_and_content`, `delivery::after_capture_plans` |
| Region / Cancel | live check pending | Start a region capture, press Esc (also try the toolbar's Cancel). Overlay closes, no file, no toast, no window when started via `annota --capture region` with Annota closed. | — beyond `geometry::click_without_drag_has_no_selection`: Esc maps to `Outcome::Cancelled`, which `capture::run` turns into "no image" before any delivery code runs (needs a display to test end to end) |
| Region / Fixed size | live check pending | After a drag, type 800 and 600 into W/H in the toolbar: the selection follows while typing. | `geometry::fixed_size_*` |
| Region / Snap to a window edge | live check pending | Drag near a window's left edge: the corner jumps within 8 px, a violet guide and "Snapped to window edge" chip appear; holding Alt disables snapping. | `snap::snaps_within_8_px`, `no_snap_beyond_8_px`, `corner_snaps_on_both_axes`, `text_does_not_snap` |
| Region / Nudge with the keyboard | live check pending | After release press →, Shift+↓; chip "at x, y" changes by 1 and 10. | `geometry::arrow_keys_nudge_one_or_ten_px`, `nudge_stops_at_monitor_edge` |
| Region (loupe) | live check pending | While dragging, the Ø 136 px loupe follows 26 px right/below the pointer and flips near the right/bottom edge; the chip shows `#RRGGBB` and `x, y`. | `geometry::loupe_flips_near_edges` |
| Window on Wayland | live check pending | Launcher → Window (or `--capture window`): GNOME's own screenshot UI appears; pick a window; it arrives like a region capture. | `portal::window_mode_uses_the_interactive_picker` |
| Multi-monitor | live check pending | Two monitors, pointer on the second, `annota --capture screen`: only that monitor's pixels. With "Capture all monitors" on: the whole desktop. | `geometry::two_monitors_map_to_image_halves`, `hidpi_layout_scales_crop`, `layout_with_negative_origin` |
| Countdown | live check pending | Launcher delay 5 s → Capture: OSD counts 5…1 (Coral ring), clicks pass through to other windows; Esc cancels, Enter captures at once. The resulting image must not contain the OSD (zoom into the centre). | `countdown::counts_down_whole_seconds`, `ring_shrinks_to_zero`, `osd_is_gone_before_the_grab` |
| Hotkey on GNOME 46 | live check pending (24.04) | Preferences › Capture shows the banner; "Create GNOME Shortcuts" → toast "3 GNOME shortcuts created"; GNOME Settings › Keyboard › Custom Shortcuts lists the three Annota entries; pressing Print starts a region capture with Annota closed. | `shortcuts::gnome_keybinding_runs_annota_capture`, `custom_keybinding_list_is_merged_not_replaced`, `d9_defaults_are_gnome_print_keys`, `program_name_prefers_path` |
| Takeover is reversible | live check pending (24.04) | After creating the shortcuts: `gsettings get org.gnome.shell.keybindings show-screenshot-ui` is `@as []`. Set Shortcut → Backspace on Region capture → it is `['Print']` again. | `shortcuts::takeover_moves_gnome_bindings_aside`, `restore_gives_keys_back`, `takeover_leaves_unrelated_keys_alone` |
| Shortcut conflict | UI checked in isolated session | Set Shortcut → press Super+L: "Used by Lock screen", Set insensitive. | `shortcuts::conflicts_are_found_by_accelerator`, `accelerators_normalise` |
| Portal shortcuts (GNOME ≥ 48) | live check pending (26.04) | First launch shows GNOME's own "allow shortcuts" dialog; afterwards Print starts a region capture while Annota runs; rows say "System shortcut". | `shortcuts::xdg_triggers_for_portal` |
| Include cursor | live check pending | Include pointer on, `annota --capture screen`: the arrow pointer is in the image where the pointer was. | `frame::pointer_is_visible_after_drawing`, `frame_without_pointer_is_untouched` |
| Paste | live check pending | Copy an image in another app, `Shift+Ctrl+V` in Annota: toast "New capture from clipboard · Saved as …" with Undo (Undo moves the file to the trash). Empty clipboard: "The clipboard does not contain an image". | — (needs a display clipboard) |
| Drop an image file / Unsupported file | live check pending | Drag a PNG from Files onto the window: dashed drop zone "Drop to Open as New Capture", then the same toast as Paste. A `.gif`: "Only PNG, JPG and WebP images can be opened". WebP needs `webp-pixbuf-loader`. | `import::accepts_png_jpg_webp_only` |
| Copy to clipboard / Both | live check pending | after-capture "Copy to Clipboard", `annota --capture region` with Annota closed: no window opens, the image pastes into another app (Annota stays running in the background until something else takes the clipboard). "Both": PNG saved and clipboard filled. | `delivery::after_capture_plans`, `imports_always_become_a_new_capture`, `unique_path_never_overwrites` |
| Portal permission handling (all scenarios) | live check pending | See "Screenshot permission" below. | `permission::the_store_entry_decides_for_this_app_only`, `not_found_is_an_answer_other_errors_are_not`, `only_an_undecided_frozen_grab_asks_first`, `only_a_stored_no_is_a_denial`, `the_reset_command_deletes_annotas_entry_only`, `portal::response_code_2_is_a_refusal_not_a_denial`; against a stand-in portal: all of `capture::live_tests` |
| Eyedropper (`portal::pick_colour`, add-core-annotation-tools) | live check pending | In the editor's colour picker, click the eyedropper swatch or "Pick From Screen": the swatch fills accent, the subtitle reads "Pick a colour anywhere on the screen", a chip shows "Click to pick · Esc cancels". Click any pixel on screen (inside or outside the Annota window) — the selected object/tool default takes that colour, added as a recent custom swatch. Esc cancels: nothing changes. The screenshot permission doesn't apply: xdg-desktop-portal's `handle_pick_color` doesn't check the store. | `portal::cancel_is_silent`, `response_code_2_is_a_refusal_not_a_denial`, `missing_portal_is_reported_as_such` (shared `map_portal_error`, same mapping as the screenshot calls); the picking UI state itself (swatch fill, subtitle, chip, recent-colour list, cancel) is covered with a stand-in in `ui::editor::panel`'s tests and its live canvas test — only the real `ashpd::desktop::Color::pick()` call is unverified |
| Second launch starts a capture (app-shell) | checked in isolated session (no portal) | With Annota running, `annota --capture region`: the existing instance starts the overlay without raising its window first; `pgrep -c annota` is 1. | — |
| Cold start: `--capture` with Annota not already running | checked in isolated session (no portal) | With no `annota` process running at all, `annota --capture region`: the process does not exit immediately — it stays alive long enough to attempt the capture (in the isolated/no-portal setup: the "Capturing needs xdg-desktop-portal" toast and window appear). Regression check for the bug where `app.hold()` ran inside the spawned future instead of before it, so a cold launch with nothing else holding the application could exit before the capture ever started. | `capture::start` holds synchronously now; needs a live re-check on a system with the portal, where a cold `--capture region` should actually open the overlay |

## Screenshot permission

**Not run on a real desktop yet.** `capture::live_tests` uses a stand-in portal written from the
sources named in `design.md`. These steps are the first real test of the fix.

Use the installed `.deb` and start Annota from the Shell (not a terminal or `meson devenv`), so
that GNOME can match it to `io.github.trosinde.Annota.desktop`. Quit Annota before each block.
These commands only touch Annota's own entry.

Read the stored answer (`No entry for screenshot` or a missing `io.github.trosinde.Annota` means none):

```sh
gdbus call --session --dest org.freedesktop.impl.portal.PermissionStore --object-path /org/freedesktop/impl/portal/PermissionStore --method org.freedesktop.impl.portal.PermissionStore.Lookup screenshot screenshot
```

Reset, so that nothing is stored for Annota:

```sh
gdbus call --session --dest org.freedesktop.impl.portal.PermissionStore --object-path /org/freedesktop/impl/portal/PermissionStore --method org.freedesktop.impl.portal.PermissionStore.DeletePermission screenshot screenshot io.github.trosinde.Annota
```

Watch the journal in a second terminal:

```sh
journalctl --user -f -g 'portal|access dialog'
```

**A. First run, choose Allow**
1. Reset. Start Annota from the Shell. Press `Ctrl+N` → Capture Region.
2. Expected: the Annota window stays on screen and GNOME asks "Allow Annota to Take Screenshots?".
   The journal has no `Only the focused app` line.
3. Choose Allow. Expected: the Annota window disappears, then the region overlay opens.
4. Zoom into the frozen image in the overlay: no Annota window and no question dialog in it.
5. Select a region and confirm: it opens in the editor. `Lookup` shows `['yes']`.
6. Capture again. Expected: no question, no window flash before the overlay.

**B. First run, choose Deny**
1. Quit Annota, reset, start Annota from the Shell, press `Ctrl+N` → Capture Region.
2. Choose Deny. Expected: no overlay; the "Screen Access Denied" dialog with the reset command.
   `Lookup` shows `['no']`.
3. Press Copy Command. Expected: the dialog closes with a "Copied to clipboard" toast. Paste the
   command into a terminal and run it. `Lookup` no longer lists Annota.
4. Capture again. Expected: GNOME asks again, as in A.

**C. Stored "no"**
1. After a Deny (B, steps 1 and 2), quit Annota, start it again and capture a region.
2. Expected: the window hides, no question appears, the window returns with the dialog.

**D. Cold start with no window, from a shortcut**
1. Quit Annota so that `pgrep annota` finds nothing. Reset.
2. Press the shortcut that runs a region capture (a GNOME custom shortcut running
   `annota --capture region`).
3. Expected: a main window opens, GNOME asks over it. Choose Allow.
4. Expected: the window goes away and the overlay opens. After Esc no Annota window remains and the
   process exits.
5. Record whether the window got the focus or GNOME showed "Annota is ready" instead, and whether the
   question still appeared. The code expects it to appear either way, because it passes a parent.

**E. Window mode**
1. Reset, start Annota, launcher → Window.
2. Expected: GNOME's own picker with no permission question, also with nothing stored. `Lookup` still
   has no entry.

**F. Could not ask**
Only if a step above ends in the toast "GNOME could not ask for screen access. …": `Lookup` must show
no entry, and there must be no "Screen Access Denied" dialog. Note the journal lines around it. That
result means the fix did not work on this desktop.

**G. Ubuntu 24.04 (GNOME 46, portal 1.18)**
Run A and B. Also check under which ID the answer is stored. If it is `''` rather than
`io.github.trosinde.Annota`, see "Known limits" in `design.md`.
