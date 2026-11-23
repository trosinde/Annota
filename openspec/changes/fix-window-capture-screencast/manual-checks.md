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
| debian-packaging / Runtime dependencies / Dependencies resolved | `docker run ubuntu:24.04`: `apt-get install --no-install-recommends ./annota_0.1.0_amd64.deb` (v0.1.0 release asset, sha256 `dde330a9…6459`) | Needs a clean Ubuntu 24.04 apt root; CI builds the package but never installs it | pass | 2026-11-22 |
| capture / Window capture / Window on Wayland | `tools/live-window-capture.py`, two runs ([live-run.md](live-run.md)) | Needs GNOME's real chooser and frames; `cargo test` only has the fake portal | pass | 2026-11-14 |
| capture / Window capture / The whole window at any display scale | `tools/live-window-capture.py -k whole`: a small, a small decorated and a maximised decorated target with four coloured corner squares at monitor scale 1.25; every corner square is in the image, whole ([live-run.md](live-run.md), 2026-11-23) | Needs GNOME's real frames and crop meta; the unit tests can only rebuild the measured units | pass | 2026-11-23 |
| capture / Window capture / Window mode from the welcome page | `tools/live-window-capture.py`, two runs ([live-run.md](live-run.md)); the fake-portal test proves only that window mode makes no Screenshot-portal call | Needs GNOME's real chooser | pass | 2026-11-14 |
| capture / Cursor inclusion / Pointer in a window capture | `tools/live-window-capture.py`, two runs ([live-run.md](live-run.md); one earlier failure in 18 runs is noted there); the fake-portal test proves only the requested `cursor_mode` | Whether the pointer is in the frame is up to the compositor | pass | 2026-11-14 |

## Helper note for 6.2 (not the live check itself)

2026-11-22, in a throwaway `ubuntu:24.04` container (24.04.5, nothing installed on the host):
`apt-get install --no-install-recommends /deb/annota_0.1.0_amd64.deb` from the v0.1.0 GitHub
release exits 0 and pulls in `gstreamer1.0-pipewire` 1.0.5-1ubuntu3.3, `gstreamer1.0-plugins-base`
1.24.2-1ubuntu0.7, `libgstreamer1.0-0` 1.24.2, `libpipewire-0.3-0t64` 1.0.5, `xdg-desktop-portal`
1.18.4, `xdg-desktop-portal-gnome` 46.2, `libgtk-4-1` 4.14.5 and `libadwaita-1-0` 1.5.0.
`ldd /usr/bin/annota` finds every library; `gst-inspect-1.0` (from `gstreamer1.0-tools`, added
only for the check) finds `pipewiresrc` (libgstpipewire.so 1.0.5), `videoconvert` and `appsink`,
the three elements of `screencast.rs`'s pipeline. `annota --version` prints `annota 0.1.0`.
This settles "Dependencies resolved" and the "elements are available" half of "Window capture
works on a fresh install". The other half, a window image on a fresh GNOME 46 desktop, stays
open for the maintainer (task 6.2): a container has no GNOME session, no PipeWire and no portal.
