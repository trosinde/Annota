# Manual checks

## Probe run (task 2.1)

Probes P1–P12 of design.md are measured with `tools/recording-probe.py` on the maintainer's own
desktop. It needs no Annota build. Two runs on GNOME 50 are logged in `probe-results/` (2026-11-23);
the probe table in design.md says which probe is measured, inconclusive or still open.
`tools/recording-probe.py selftest` checks the frame analysis offline (videotestsrc, no portal).

Before you start:
- Quit Annota (`pkill -x annota`). Annota must not run during the probes: GNOME delivers
  shortcuts to every bound session, and only one screen-sharing session should be open.
- Run it from the repository root, in a terminal on the desktop under test (not over SSH).
- Each probe explains itself and waits for Enter. Press `s` to skip a probe.
- Every run appends its results to
  `openspec/changes/add-screen-recording/probe-results/<date>-<os>-<session>.md`. Commit that
  file. Do **not** commit the PNGs written to `/tmp` (they contain your screen).

| # | Command | What you do | Dialogs and indicators you will see | Measures |
|---|---|---|---|---|
| 1 | `tools/recording-probe.py env` | nothing | none | versions, `AvailableSourceTypes`/`AvailableCursorModes`, GDK monitors and scales, which package provides each GStreamer element (task 2.3) |
| 2 | `tools/recording-probe.py audio` | play some sound during the 2nd half | none; GNOME's microphone indicator for about 3 s | default source, `@DEFAULT_MONITOR@`, devices, levels (task 2.2) |
| 3 | `tools/recording-probe.py monitor` | choose a screen, then Share | GNOME's screen-sharing chooser (screen list) with a small "Annota recording probe" window as its parent; then GNOME's sharing indicator for 3 s | P1 (chooser shown, code, time), P2 (`position`/`size` vs GDK monitors), P3 (buffer vs `size`, crop meta), P10 (parent) |
| 4 | `tools/recording-probe.py monitor --no-parent` | same as 3 | same chooser, no probe window | P10 (without parent: does the chooser still appear?) |
| 5 | `tools/recording-probe.py monitor --expect cancel` | press Cancel (or Esc) in the chooser | GNOME's screen-sharing chooser | P1 (response code and time of a cancelled chooser) |
| 6 | `tools/recording-probe.py cursor-modes` | for each offered mode: choose a screen, keep the pointer on it; afterwards open `/tmp/annota-probe-cursor-*.png` and note whether the pointer is drawn | chooser once per cursor mode (up to 3 times), sharing indicator | P4 |
| 7 | `tools/recording-probe.py monitor --save /tmp/annota-probe-frame.png` | choose the screen that shows GNOME's panel; afterwards look at the PNG (it shows your screen: do not commit or share it) | chooser, sharing indicator | P12 (is GNOME's indicator in the stream?): answered from the P4 frames, optional |
| 8 | `tools/recording-probe.py session-close` | choose a screen, then **press the small square ■ next to the screen-cast icon in GNOME's panel** (bottom panel on the maintainer's desktop, between the clipboard and the accessibility icons) | chooser, sharing indicator (screen-cast icon + ■, which you use to stop) | P5 (Session.Closed, stream end, buffers per second until the end) |
| 9 | `tools/recording-probe.py window-end` | choose a window you can close that is not the terminal (e.g. a new Text Editor window); follow the printed countdown: resize until 20 s, hands off, close it at the "CLOSE THE WINDOW NOW" prompt (30 s), then wait for the end (60 s) | GNOME's screen-sharing chooser (window list), sharing indicator | P6 (buffer size and crop meta over time, buffers and content changes per second, repeated frames after the close; a `Session.Closed` during Start ends the probe at once with its own verdict). Downscaled frames go to `/tmp/annota-probe-p6-*.png` (first, each size/crop change, after the close, last) for the coordinating session to look at; you are not asked what you saw. They show the chosen window: do not commit them |
| 10 | `tools/recording-probe.py frame-rate` | twice (keepalive off and 1000 ms): first stop every video and animation; after the 3-2-1 keep the pointer still for 30 s, after the next 3-2-1 scroll a page or drag a window for 30 s | chooser twice, sharing indicator | P7 (informational: the pipeline handles a variable frame rate either way, design.md D3) |
| 11 | `tools/recording-probe.py load --minutes 30` | work normally on the largest monitor, a playing video helps | chooser, then the sharing indicator for 30 min. Nothing is written | P8 (encoded fps, videorate drops, CPU, RSS); moved to Verify (task 6.2), measured with the real implementation |
| 12 | `tools/recording-probe.py shortcuts` | press `Super+Alt+R` while it watches (15 s) | possibly GNOME's "allow global shortcuts" dialog listing Region, Window, Screen and **Record video**. **Lasting effect:** GNOME stores the `record` binding for Annota's app ID, as Annota itself will at start | P11 (reported trigger, "Used by GNOME" via read-only GSettings lookup, delivery) |
| 13 | P9 on Ubuntu 24.04: log in with "GNOME on Xorg", run 1, 3 and 9 | as above | as above, if X11 offers ScreenCast at all | P9 |

`tools/recording-probe.py all` runs rows 1, 2, 3, 5, 6, 8, 9, 10 and 12 one after another
(about 10 minutes plus the 60 s waits). Run rows 4, 7 and 11 separately. Repeat on Ubuntu 24.04
(GNOME 46) where a machine is available (task 2.1c).

## Live checks (filled during apply)

| Spec | Check | Why not automated | Result | Date |
|---|---|---|---|---|
