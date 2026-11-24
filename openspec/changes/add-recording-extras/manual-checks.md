# Manual checks

## Probe run (task 2.1)

Probes E1–E7 of design.md are measured on the maintainer's own desktop once the core
(`add-screen-recording`) exists and its probes P1–P12 have been run. They rely on P2/P3 (monitor
mapping), P4 (cursor modes) and P7 (frame delivery). **No probe tool exists yet.** Task 2.1 adds
an `extras` group to `tools/recording-probe.py`. The commands below are the planned interface.
Each one explains itself before it starts and waits for Enter; `s` skips a probe.

Before you start:
- Quit Annota (`pkill -x annota`), as for the core probes.
- Run the commands from the repository root, in a terminal on the desktop under test (not over
  SSH).
- Each run appends to `openspec/changes/add-recording-extras/probe-results/<date>-<os>-<session>.md`.
  Commit that file. Do **not** commit PNGs or camera frames written to `/tmp`: they contain your
  screen or your face.
- Run on GNOME 50 (own desktop) and, where possible, GNOME 46 (Ubuntu 24.04). E1 and E3 also run
  on "GNOME on Xorg".

| # | Command (planned) | What you do | Dialogs and indicators you will see | Measures |
|---|---|---|---|---|
| E1 | `tools/recording-probe.py extras cursor-meta` | choose a screen, move the pointer for 10 s; then the same with a window | GNOME's screen-sharing chooser (screen list, then window list), sharing indicator | `AvailableCursorModes` contains Metadata (4) for monitor and window sources; whether `pipewiresrc` exposes `SPA_META_Cursor` to GStreamer or an own PipeWire consumer is needed (design.md D4, O3) |
| E2 | `tools/recording-probe.py extras cursor-meta --static` | choose a screen, keep everything still for 15 s, then move only the pointer for 15 s; repeat at 125 % scaling if available | chooser, sharing indicator | units (stream px vs logical) at 1.0 and 1.25, hotspot, bitmap on every buffer or only on change, metadata-only buffers on a static screen and their rate |
| E3 | `tools/recording-probe.py extras draw-surface` | choose a screen; a transparent full-screen probe window opens on it; draw with the mouse for 10 s, try to click the probe's small control window, press `Esc`; afterwards open `/tmp/annota-probe-draw-*.png` | chooser, sharing indicator, a transparent window covering that monitor (the top bar included, if GNOME allows it) | does the surface cover the monitor, does it take all pointer input, does the control stay visible and clickable above it, is the surface in the monitor stream, offset and latency between the surface's strokes and strokes composited at the mapped stream position (design.md D5, O1 (a)/(b)), 2 px outline outside a region placed correctly (O4) |
| E4 | part of E3 | press `A`, `R`, `N`, `C` right after the surface opens | as E3 | whether keyboard focus reaches the surface at once (focus-stealing prevention) |
| E5 | `tools/recording-probe.py extras camera` | nothing; then once more while another app (e.g. Cheese or a browser call) holds the camera | GNOME's camera indicator for about 3 s; possibly a camera permission dialog — note it | `GstDeviceMonitor` `Video/Source`, `v4l2src` vs `pipewiresrc` camera node, formats, latency, behaviour when the camera is busy, any permission dialog for a native app (design.md D2) |
| E6 | `tools/recording-probe.py extras load --minutes 10` | work normally on the largest monitor, camera on | chooser, sharing and camera indicators for 10 min. Nothing is written | dropped frames, CPU, RSS for 4K stream + 720p camera + `compositor` + `cairooverlay` + `x264enc superfast` (extends core P8) |
| E7 | `tools/recording-probe.py extras env` (also in clean Ubuntu 24.04, 26.04, Debian 13 containers, task 2.3) | nothing | none | presence and owning package of `compositor`, `cairooverlay`, `alpha`, `v4l2src`, `gtk4paintablesink`, `avenc_gif` (design.md D1, D6, D8) |

After the run: correct spec and design wherever a result contradicts them (task 2.2). Report the
E3 result to the maintainer for O1 (a) vs. (b). If E1 is negative, take O3 to the maintainer
before task group 3.

## Live checks (filled during apply)

| Spec | Check | Why not automated | Result | Date |
|---|---|---|---|---|
