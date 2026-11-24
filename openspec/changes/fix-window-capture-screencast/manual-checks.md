# Manual checks

Every scenario of the `capture` delta's "Window capture" and "Cursor inclusion" (window part)
runs automatically in `tools/live-window-capture.py` ([live-run.md](live-run.md)), except
"No frame arrives" and "No window source", which a working GNOME desktop cannot produce; the
fake portal in `src/capture/live_tests.rs` covers them. What stays manual:

| Spec | Check | Why not automated | Result | Date |
|---|---|---|---|---|
| debian-packaging / Runtime dependencies / Window capture works on a fresh install | Fresh Ubuntu 24.04 with GNOME: `apt install ./annota_*.deb`, Window card → chooser → editor with "Window captured · W × H" | Needs a fresh VM with a GNOME session | open (not run 2026-11-23: needs a fresh Ubuntu 24.04 GNOME install, task 6.2) | |
| capture / Window capture / No window source | GNOME on X11: `AvailableSourceTypes` of the ScreenCast portal, and window mode falls back to the Screenshot UI | No X11 session on the dev machine (design.md D6: unmeasured) | open (not run 2026-11-23: the desktop is Wayland only) | |
| capture / After-capture destination / Copy to clipboard on Wayland | `annota --capture region` with after-capture "clipboard", drag + `Enter`: `wl-paste --list-types` lists `image/png`, toast says "Copied to clipboard" | Needs mutter's real focus and serial rules; the unit test covers only the decision | pass | 2026-11-14 |
| capture / After-capture destination / A copy that cannot be made | `tools/live-window-capture.py` "Window on Wayland": the toast's claim matches the clipboard, Copy then copies | Live suite, outside `cargo test` | inconclusive: the toast's claim matched the clipboard in every run, but in 9 of about 47 captures the toast's Copy (pressed over AT-SPI after one synthetic `Shift_L`) put nothing on the clipboard, even 20 s later. Whether a real click copies was not tested; see "Live run 2026-11-23" | 2026-11-23 |
| debian-packaging / Runtime dependencies / Dependencies resolved | `docker run ubuntu:24.04`: `apt-get install --no-install-recommends ./annota_0.1.0_amd64.deb` (v0.1.0 release asset, sha256 `dde330a9…6459`) | Needs a clean Ubuntu 24.04 apt root; CI builds the package but never installs it | pass | 2026-11-22 |
| capture / Window capture / Window on Wayland | `tools/live-window-capture.py`, two runs ([live-run.md](live-run.md)) | Needs GNOME's real chooser and frames; `cargo test` only has the fake portal | pass | 2026-11-14 |
| capture / Window capture / The whole window at any display scale | `tools/live-window-capture.py -k whole`: a small, a small decorated and a maximised decorated target with four coloured corner squares at monitor scale 1.25; every corner square is in the image, whole ([live-run.md](live-run.md), 2026-11-23) | Needs GNOME's real frames and crop meta; the unit tests can only rebuild the measured units | pass | 2026-11-23 |
| capture / Window capture / Window mode from the welcome page | `tools/live-window-capture.py`, two runs ([live-run.md](live-run.md)); the fake-portal test proves only that window mode makes no Screenshot-portal call | Needs GNOME's real chooser | pass | 2026-11-14 |
| capture / Cursor inclusion / Pointer in a window capture | `tools/live-window-capture.py`, two runs ([live-run.md](live-run.md); one earlier failure in 18 runs is noted there); the fake-portal test proves only the requested `cursor_mode` | Whether the pointer is in the frame is up to the compositor | pass | 2026-11-14 |

## Live run 2026-11-23

GNOME Shell 50 on Wayland, one monitor 3840 × 2160 at scale 1.25 (logical 3072 × 1728). Annota
built from `main` b96c775 out of tree (`meson setup … -Dprofile=development`), run with private
keyfile settings by `tools/live-window-capture.py --annota <build>/src/annota`, which drives the
chooser through `tools/portal-input.py` with no human. A scratch copy of the tool also wrote every
clipboard image to disk so the pixels could be checked by hand.

Second full run (`--runs 2`):

| Scenario | Run 1 | Run 2 |
|---|---|---|
| capture / Window capture / Window on Wayland | pass | pass |
| capture / Window capture / Window mode from the welcome page | pass | pass |
| capture / Window capture / Waiting for the chooser | pass | pass |
| capture / Window capture / Cancel while waiting | pass | pass |
| capture / Window capture / Chooser closed | pass | pass |
| capture / Window capture / Delayed window capture | pass | pass |
| capture / Window capture / Countdown cancelled | pass | pass |
| capture / Cursor inclusion / Pointer in a window capture | pass | pass |
| capture / Cursor inclusion / Cursor excluded | pass | pass |

The first full run was the same except "Pointer in a window capture", run 2: FAIL, "no image on
the clipboard". The suite's verdicts are not the whole result:

**Defect (on b96c775, fixed by PR #12 — see the row "The whole window at any display scale"): the window image is only the window's top-left part (scale 1.25).** The target window
is uniformly magenta, so a wrong cut still passes. With a labelled or decorated GTK 4 window:

- Small decorated window (640 × 400 logical): toast "Window captured · 828 × 528". The image
  shows the window's rounded top-left corner and its top and left outline, but no right or bottom
  outline and no other corner. The title, centred in the window, sits at x 517–764 of 828, so the
  window is drawn about 1.6 times larger than the image.
- Maximised decorated window (3072 × 1680 logical): toast "Window captured · 3840 × 2100"; the
  centred title sits at x 2947–3194, centre 3070, where 1920 is expected.
- `tools/portal-probe.py --screencast-window` on the same windows measures why. The buffer is
  6144 × 3456 (scale 2), but the crop meta is in scale-1.25 pixels: undecorated 640 × 400 window,
  crop meta (0, 0, 800, 500), alpha bounding box (0, 0, 1280, 800); decorated, crop meta
  (0, 0, 928, 628), alpha box (52, 65, 1378, 906); maximised, crop meta (0, 0, 3840, 2100), alpha
  box (0, 0, 6144, 3360). D3 assumed the crop meta is in buffer pixels, as measured on
  2026-11-14 for a terminal, (0, 0, 2524, 2032). `window_rect` searches only inside the crop meta
  and returns its top-left 62.5 % of width and height (39 % of the window).
- What still holds: the image has no transparent margin or shadow (the small undecorated window
  gave 800 × 500, every pixel opaque magenta), the size in the toast matches the window's size
  at 1.25, and both a small and a maximised window come through.

**Clipboard read-back is flaky (harness or product, not settled).** When the toast offers Copy,
the tool sends one synthetic `Shift_L`, presses Copy over AT-SPI and waits for `image/png`. In 9
of about 47 captures nothing reached the clipboard, even 20 s later. Annota logged "the capture
was not copied (focused: true, input seen: false)" in passing and failing runs alike. A Copy
pressed over AT-SPI carries no input serial of its own, so this does not show that a real click
fails.

**Pointer placement in the tool.** After `move(-10000, 10000)`, the x part of the following
`move(160, -400)` is lost: a motion-logging target window saw the pointer at (0, 1327), not
(160, 1328). The arrow is therefore at the left screen edge and partly clipped (bbox x 0–11 in
the image). It is still detected, but it sits only a few pixels from failing. One run reported "no
pointer"; that image was not kept. During the session someone also moved the real mouse (long
motion paths in the log), which may explain some flakes.

6.1 items, 2026-11-23:

| Item | Result |
|---|---|
| Window card → chooser → editor with "Window captured · W × H", no transparent margin, no shadow | toast and size pass, no margin or shadow; **image content failed on b96c775, fixed by PR #12** (top-left part only, above) |
| `annota --capture window` from a terminal | pass (the suite starts it exactly so, 4 of 4 runs) |
| Launcher "Capture Window", GNOME keybinding | not run: the launcher and keybinding start the installed `/usr/local/bin/annota` from 2026-11-14, not this build |
| Chooser closed: nothing happens | pass, 4 of 4 |
| Delay 3 s: chooser, countdown, frame | pass, 4 of 4 (frame 4.5 s after the choice) |
| Countdown cancelled | pass, 4 of 4 (no image, session closed) |
| Include pointer on / off | on: pass whenever an image arrived; off: pass, all pixels magenta |
| Scale 1.25 / 1 | 1.25 run (and fails, above); scale 1 not possible on this desktop |
| Maximised and small window | both captured at the right size; both show the crop defect |

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
