# Manual checks

Every scenario of the `capture` delta's "Window capture" and "Cursor inclusion" (window part)
runs automatically in `tools/live-window-capture.py` ([live-run.md](live-run.md)), except
"No frame arrives" and "No window source", which a working GNOME desktop cannot produce; the
fake portal in `src/capture/live_tests.rs` covers them. What stays manual:

| Spec | Check | Why not automated | Result | Date |
|---|---|---|---|---|
| debian-packaging / Runtime dependencies / Window capture works on a fresh install | Fresh Ubuntu 24.04 with GNOME: `apt install ./annota_*.deb`, Window card → chooser → editor with "Window captured · W × H" | Needs a fresh VM with a GNOME session | open | |
| capture / Window capture / No window source | GNOME on X11: `AvailableSourceTypes` of the ScreenCast portal, and window mode falls back to the Screenshot UI | No X11 session on the dev machine (design.md D6: unmeasured) | open | |
| capture / After-capture destination / Copy to clipboard on Wayland | `annota --capture region` with after-capture "clipboard", drag + `Enter`: `wl-paste --list-types` lists `image/png`, toast says "Copied to clipboard" | Needs mutter's real focus and serial rules; the unit test covers only the decision | pass | 2026-11-14 |
| capture / After-capture destination / A copy that cannot be made | `tools/live-window-capture.py` "Window on Wayland": the toast's claim matches the clipboard, Copy then copies | Live suite, outside `cargo test` | pass | 2026-11-14 |
