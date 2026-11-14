# Manual checks — fix-portal-shortcut-bindings

Live checks on the maintainer's desktop (GNOME Shell 50, xdg-desktop-portal 1.21, GlobalShortcuts
portal version 1; the fallback rows on a GNOME 46 system, or with `gnome-shortcuts-created` true).
Run a fresh build with `meson devenv -C _build ./src/annota`, after quitting any running Annota.
The automated side is in `src/capture/live_tests.rs` (fake portal) and the unit tests of
`src/capture/shortcuts.rs` and `src/preferences.rs`; these rows cover what only the real desktop
can show. Task 6.1 adds the remaining rows before archive.

| Spec | Check | Why not automated | Result | Date |
|---|---|---|---|---|
| capture / Global capture hotkey / Open GNOME Settings | Portal path: Preferences › Capture shows "Open GNOME Settings" in the "Global Shortcuts" header. Click it: GNOME Settings comes to the front at Apps › Annota, whose "Global Shortcuts" row lists Region, Window and Screen capture. Then remove `org.gnome.Settings.desktop` from the search path (e.g. `XDG_DATA_DIRS` without `/usr/share` for a test launch) and reopen Preferences: the button is gone, the description is unchanged. (Task 4.2.) | Needs the real GNOME Settings and its `launch-panel` action; the tests may not start desktop applications. The D-Bus parameters are unit-tested (`open_gnome_settings_targets_annotas_apps_page`). | not run | |
| capture / Reversible takeover of GNOME's screenshot keys / Conflict with an Annota application shortcut | Fallback path (after "Create GNOME Shortcuts"): Preferences › Capture › Screen capture, press `Ctrl+Q` in the Set Shortcut dialog. The status line reads "Used by Annota: Quit" in the error style with the warning icon, the keycaps `Ctrl` `Q` have the red ring, and "Set" stays insensitive (mockup 04). Repeat with `Ctrl+O` ("Used by Annota: Open Image") and `Ctrl+S` ("Used by Annota: Save", also before any editor was opened). In German: "Belegt durch Annota: Beenden". (Task 5.2.) | A key press into the dialog's capture-phase key controller cannot be sent from the headless tests; the decision is unit-tested (`annotas_own_accelerators_are_conflicts`). | not run | |
