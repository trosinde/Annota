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
| 9 | `tools/recording-probe-drive.py window-end` (no human: the driver starts a GTK target window, chooses it in GNOME's chooser via the RemoteDesktop token + AT-SPI, maximizes, unmaximizes and closes it). By hand: `tools/recording-probe.py window-end` and follow the countdown with an ordinary window, not the terminal | do not touch keyboard or mouse while the driver runs; run `tools/portal-input.py check` once beforehand | GNOME's screen-sharing chooser (window list), operated by the driver; sharing indicator | P6 (buffer size and crop meta over time, buffers and content changes per second, `Session.Closed` on close; downscaled frames in `/tmp/annota-probe-p6-*.png`, not to be committed). Measured 2026-11-23 20:48 |
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
| `screen-recording / Recording source / Record the screen` | `tools/live-recording.py --dev run screen` (3 runs): `--record screen`, chooser driven, 4K monitor at scale 1.25 | real ScreenCast portal and GNOME chooser | pass: frames 3840×2160 at 30/1 fps = the stream's full buffer size | 2026-11-24 |
| `screen-recording / Recording on the command line / Record a screen from the command line` | same run: indicator and countdown observed over AT-SPI | real ScreenCast portal and GNOME chooser | pass: "Waiting for a screen…" shown with the chooser, gone 0.28 s after Share, then "Recording starts in 3 s" | 2026-11-24 |
| `screen-recording / Recording countdown / Countdown is not recorded` | screen run, countdown 3: OSD timed over AT-SPI, first frame of the MP4 inspected | real ScreenCast portal and GNOME chooser, real compositor | pass: OSD shown 2.98 s; first frame shows neither the OSD nor the waiting indicator | 2026-11-24 |
| `screen-recording / Recording countdown / Waiting indicator is not recorded` | first frames of the screen (countdown 3), region (1) and window/desktop-stop (0) runs | real ScreenCast portal and GNOME chooser, real compositor | pass: no indicator, OSD or region outline in any first frame | 2026-11-24 |
| `screen-recording / Recording countdown / No countdown` | window and desktop-stop runs with countdown 0 | real ScreenCast portal and GNOME chooser | pass: no countdown label seen; control 0.55 s after the indicator closed | 2026-11-24 |
| `screen-recording / Pause and resume / Paused time is not recorded` | screen run: ~8.65 s recorded, Pause 3.0 s, ~4.0 s recorded, stop by `--record toggle`; `gst-discoverer-1.0` on working file and MP4 | real portal stream timing | pass: 12.50 s (working file) / 12.53 s (MP4) against 12.66 s from the driver's timestamps (−0.13 s, AT-SPI timing uncertainty); 15.7 s would include the pause | 2026-11-24 |
| `screen-recording / Pause and resume / Paused state` | screen run: control read over AT-SPI 1.5 s and 3 s into the pause | real recording session | pass: "Paused" shown, Resume button, time frozen at 0:08 | 2026-11-24 |
| `screen-recording / Recording control / Elapsed time` | screen run: time label read before the stop | real recording session | pass: 0:12 after 12.5 s recorded | 2026-11-24 |
| `screen-recording / Audio / Mute system audio` | screen run: quiet 880 Hz tone to the default sink throughout; Mute system audio (AT-SPI) 2.5 s; `level` peaks per 0.5 s of the system track | real audio devices (PipeWire) | pass: −26 dB before and after, −350 dB (silence) in the windows 4.6–6.1 s for a mute from 4.13 to 6.63 s | 2026-11-24 |
| `screen-recording / Audio / Mute the microphone` | same run, Mute microphone 1.5 s after resume; `level` of the microphone track | real audio devices (PipeWire) | pass: silent 10.1–10.6 s windows for a mute from 9.65 to 11.15 s, −26 dB around it. Note: this desktop has no default input set, the default source is the HDMI sink's monitor, so the "microphone" track carried the test tone too | 2026-11-24 |
| `screen-recording / Audio / No audio selected` | region run with Microphone and System audio off | real devices | pass: MP4 has one H.264 track, no audio | 2026-11-24 |
| `screen-recording / Recording source / Record a region on a scaled monitor` | `run region`: 800×600 logical at (1050, 450) dragged by the RemoteDesktop pointer over a full-screen grid (line every 100 logical px) | real monitor stream, scale 1.25 | pass: overlay showed 1000 × 750; frames 1000×750; grid lines at columns/rows 62–63, 187–188 … = (1100−1050)×1.25 etc., so exactly that area | 2026-11-24 |
| `screen-recording / Recording source / Pointer in the recording` | pointer moved over the recorded area: region and screen runs Include pointer on, window and desktop-stop runs off | real compositor cursor modes | pass: arrow drawn at (1450, 750) logical in the region frames and in the screen frames; no pointer in the window or desktop-stop frames | 2026-11-24 |
| `screen-recording / Recording source / Record a window` | `run window`: magenta 640×400 GTK target chosen in the chooser (3 runs before D2a; 1 run at 04:38 after D2a, frame size in monitor pixels) | real ScreenCast portal and GNOME chooser (window list) | pass: 04:38 run frames 802×502 (the 640×400 window incl. its 1 px outline at scale 1.25, D2a), only the window, no shadow, no desktop around it; earlier 3 runs 1284×804 at the old buffer-scale size | 2026-11-24 |
| `screen-recording / Recording source / Recorded window resized` | window run: target maximized for 3 s, then unmaximized; the target redraws 1.5 s after each size change (as real apps do); frames at 1 fps (`size-window-strip.png`, not committed) | real window stream (crop meta, GNOME's maximize/unmaximize animation frames, P6) | pass: 04:38 run (after the per-buffer fix and D2a): frames stay 802×502; maximized whole window letterboxed (black bands top and bottom), not stretched; after unmaximize the whole window with its header bar, framed as at the start. Between a resize and the window's next redraw GNOME streams its maximize/unmaximize animation frame (1 frame per resize in the 1 fps strip), a GNOME limitation recorded in design.md. History: failed 3/3 earlier on 2026-11-24 before the fix (rectangle cached per crop meta from the animated buffer) | 2026-11-24 |
| `screen-recording / Stop recording / Recorded window closed` | window run: target closed after ~11 s; `dbus-monitor` for `Session.Closed`, GST_DEBUG basesrc/pipewiresrc | real portal end of a window stream | pass: `Session.Closed` 0.02–0.03 s after the close, Export Recording 0.28–0.31 s after; pipewiresrc ended by flush, no EOS from the stream; 11.33–11.40 s kept | 2026-11-24 |
| `screen-recording / Stop recording / Session ended by the desktop` | `run desktop-stop`: GNOME's "Stop Screen Sharing" (■) in the panel, clicked with the RemoteDesktop pointer at its AT-SPI extents | real GNOME panel indicator | pass: `Session.Closed` at once, Export Recording 0.34 s later, no EOS from pipewiresrc, 6.27–6.37 s kept. ■ also ends the test tool's RemoteDesktop session | 2026-11-24 |
| `screen-recording / Stop recording / Stop by button` | region run: Stop in the control (AT-SPI) | real recording session | pass: control gone, Export Recording 0.5 s later | 2026-11-24 |
| `screen-recording / Stop recording / Stop by hotkey` | screen run: `annota --record toggle`, the action the Record shortcut runs (the real key was not pressed: not bound, see note) | real recording session | pass (toggle path only): Export Recording 0.35 s later | 2026-11-24 |
| `screen-recording / Export after recording / Export as MP4` | screen run: Export with MP4 | real recording | pass: H.264 3840×2160 30/1 + one AAC track (both sources mixed), 12.53 s; "Saved" shown, Show in Files present (not activated), working file removed | 2026-11-24 |
| `screen-recording / Export after recording / Export as WebM` | window run: Export with WebM (also at 04:38) | real recording | pass: VP9 802×502 + Opus, 11.27 s (04:38); earlier VP9 1284×804 + Opus, 11.33 s; "Saved" after about 1 s (progress bar not observed at that speed) | 2026-11-24 |
| `screen-recording / Discard a recording / Discard` | desktop-stop run: Discard, then Discard in "Discard Recording?" | real recording | pass: working file deleted, window closed | 2026-11-24 |
| `debian-packaging / Recording runtime dependencies / Recording elements installed` | CI's `.deb (lintian)` artifact of run 45311800952 (`annota_0.1.0_amd64.deb`, built on ubuntu-24.04 with Rust 1.91) installed with `apt-get install ./annota_0.1.0_amd64.deb` in a clean `ubuntu:24.04` container (universe on), then `gst-inspect-1.0` for every element of the requirement plus `h264timestamper` and `openh264dec` | needs a clean system and the built package; no Rust test can install a .deb | pass: all 17 elements found (pipewiresrc, pulsesrc, level, audiomixer, videocrop, videorate, x264enc, opusenc, matroskamux, matroskademux, h264parse, h264timestamper, openh264dec, mp4mux, vp9enc, webmmux, voaacenc); Ubuntu 26.04 and Debian 13 by package contents in `probe-results/2026-11-24-containers-elements.md` | 2026-11-24 |

**Window frame size (D2a, maintainer 2026-11-24):** the first window runs predate the decision that window recordings come out at the window's on-screen size in monitor pixels (their 1284×804 frames were the buffer-scale size); the 04:38 run re-checked the window rows at 802×502.

Live checks run with `tools/live-recording.py --dev` (development build, private keyfile settings).
Not run: the Record shortcut by its real key. With fresh settings GNOME shows its "Add Keyboard
Shortcuts" dialog for every start (Record video proposed as `Alt+Super+R`, so the preferred
`ALT+LOGO+r` is accepted; the capture triggers are reassigned to Ctrl+Print, Shift+Ctrl+Print,
Ctrl+Alt+Print). The tool cancels it (Add would store bindings in the user's GNOME settings);
BindShortcuts then answers 2 and Annota logs "the global shortcuts portal did not bind", the Record
card shows no trigger, so no key was pressed.
