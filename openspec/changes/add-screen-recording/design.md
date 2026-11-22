# Design: Add screen recording (core)

## Mockups (design gate)

Mockups live in `docs/design/add-screen-recording/` (designer agent, redrawn 2026-11-22 for the
core scope). Scenario-to-mockup mapping, redlines and platform notes:
[`docs/design/add-screen-recording/index.md`](../../../docs/design/add-screen-recording/index.md).

| # | Mockup | Shows |
|---|---|---|
| 01 | [recording-setup](../../../docs/design/add-screen-recording/png/01-recording-setup-light@1x.png) | Region overlay in Video mode, recording setup popover |
| 02 | [setup-unavailable](../../../docs/design/add-screen-recording/png/02-setup-unavailable-light@1x.png) | No microphone, source not offered |
| 03 | [waiting-for-screen](../../../docs/design/add-screen-recording/png/03-waiting-for-screen-light@1x.png) | Waiting indicator while GNOME's screen chooser is open |
| 04 | [countdown](../../../docs/design/add-screen-recording/png/04-countdown-light@1x.png) | "Recording starts in 3 s" |
| 05 | [recording-control](../../../docs/design/add-screen-recording/png/05-recording-control-light@1x.png) | Control window while recording, states band |
| 06 | [paused-mic-muted](../../../docs/design/add-screen-recording/png/06-paused-mic-muted-light@1x.png) | Paused, microphone muted |
| 07 | [export-recording](../../../docs/design/add-screen-recording/png/07-export-recording-light@1x.png) | Export Recording window after stop (D1) |
| 08 | [export-progress](../../../docs/design/add-screen-recording/png/08-export-progress-light@1x.png) | WebM export in progress |
| 09 | [export-saved](../../../docs/design/add-screen-recording/png/09-export-saved-light@1x.png) | "Saved", Show in Files |
| 10 | [discard-confirm](../../../docs/design/add-screen-recording/png/10-discard-confirm-light@1x.png) | "Discard Recording?" |
| 11 | [recording-error](../../../docs/design/add-screen-recording/png/11-recording-error-light@1x.png) | Recording stopped by an error |
| 12 | [welcome-record-card](../../../docs/design/add-screen-recording/png/12-welcome-record-card-light@1x.png) | Welcome page with the Record card |
| 13 | [region-not-on-screen](../../../docs/design/add-screen-recording/png/13-region-not-on-screen-light@1x.png) | Toast when the region is not on the chosen screen |
| 14 | [preferences-record-shortcut](../../../docs/design/add-screen-recording/png/14-preferences-record-shortcut-light@1x.png) | "Record video" row in Preferences › Capture |

Dark and 2× renders sit next to each file (`-dark@1x.png`, `@2x.png`). The out-of-scope screens
of the first set moved to `docs/design/add-recording-extras/`.

Design approved: _pending_

## Context

The maintainer decided on 2026-11-22 to split video: this change carries the basic functions,
`add-recording-extras` everything else. Recording reuses the capture flows instead of building
parallel ones: the region overlay (`capture` "Region capture"), the ScreenCast window chooser and
its waiting indicator (`fix-window-capture-screencast`, ADR 0002), the countdown OSD
(`capture` "Delayed capture", `src/capture/countdown.rs`), the Include-pointer setting
(`capture` "Cursor inclusion") and the shortcut machinery (`capture` "Global capture hotkey").

## Decisions

### D1 — After stop: the Export Recording window (PROPOSED, needs the maintainer's approval)

Without the video editor there is nothing to edit, but the user still has to pick a format and
a place. Proposal: stopping opens a small **Export Recording** window at once (mockup 07):
poster frame, duration, file name, folder, MP4 | WebM, Include audio, Discard / Export. Export
shows progress (08) and ends in "Saved" with Show in Files (09). Discard and closing the window
ask first (10). The working file (D3) is removed after a successful export or a discard.

Why: the format is chosen after seeing what was recorded; MP4 is a fast remux (D3); the window
gives every failure (11) and every "Saved" a place to be shown even when no other Annota window
is open; it is the natural spot where `add-recording-extras` later puts "Edit…" (video editor).

Alternatives considered:
- **A — Save straight away + toast/notification "Show in Files".** Format chosen before
  recording (setup row), file written directly as MP4/WebM. Fewest clicks; no export wait for
  MP4. Costs: WebM must be encoded live (VP9 is slow, risk of dropped frames at 4K); a toast
  needs a window (or a GNOME notification, whose actions need D-Bus activation); no point to
  discard a failed take except the file manager.
- **B — Recording opens a preview window** with a player and Export/Discard. Better review
  before keeping, but needs a player widget (`GtkVideo`/GStreamer sink) — closer to the video
  editor of `add-recording-extras`, so it would pull scope back in.

### D2 — Sources

- **Region**: the existing region overlay (Screenshot-portal frozen image, unchanged) gets a
  Screenshot | Video mode pill; Video shows the setup. Start Recording closes the overlay and
  requests a ScreenCast **monitor** source (`types` = MONITOR, `multiple` = false, no
  `persist_mode`). The stream is cropped (`videocrop`) to the selection. The selection is
  in overlay (logical) coordinates; the mapping to stream pixels uses the stream's `position`
  and `size` and the buffer size — all unmeasured (probes P2, P3). If the chosen monitor does
  not contain the whole selection, the session is closed with a toast (mockup 13).
  Rejected alternative: ScreenCast first and the overlay drawn on the stream's first frame. That
  avoids the coordinate mapping but changes the capture flow and the order of dialogs; revisit if
  P2/P3 show the mapping is unreliable.
- **Window**: `capture`'s window flow (chooser + "Waiting for a window…"), but the stream is kept
  and recorded instead of one frame. The window buffer arrives larger than the window with a
  transparent margin (measured 2026-11-14 for one frame) — the crop rule of
  `screencast::window_rect` has to hold per frame, and a resize mid-recording must be measured
  (P6). Frames are scaled to the first frame's size, letterboxed, never stretched.
- **Screen**: monitor source, recorded whole. Annota cannot preselect the monitor under the
  pointer; the desktop's chooser decides.
- No restore token anywhere, as for window stills: the chooser is the user's consent each time.
- Pointer: `cursor_mode` Embedded when Include pointer is on and `AvailableCursorModes` offers
  it, else Hidden (`screencast::cursor_mode`, reused).

### D3 — Pipeline and working file

```
pipewiresrc ─ videoconvert ─ [videocrop] ─ videoscale ─ videorate(30 fps) ─ x264enc ─┐
pulsesrc (default source)          ─ volume(mute) ─ level ─ opusenc ──────────────────┼─ matroskamux ─ filesink
pulsesrc (default sink .monitor)   ─ volume(mute) ─ level ─ opusenc ──────────────────┘
```
- Working file: Matroska in the recordings folder (`~/Videos/Annota`), one video track and one
  Opus track per enabled audio source. Matroska stays playable when the process dies (scenario
  "Recording survives a crash", verified by a test that kills the pipeline). `x264enc`
  `speed-preset=superfast`, `tune=zerolatency` off, constant quality (`pass=quant`,
  `quantizer≈20`, tuned in task 2.x against CPU load at 4K).
- Export MP4: **remux** the H.264 track unchanged (`matroskademux ! h264parse ! mp4mux`), mix
  the audio tracks (`audiomixer`) and encode AAC → fast, no video quality loss. `mp4mux` with
  `faststart`.
- Export WebM: transcode to VP9 (`vp9enc`, `row-mt`, `deadline=1`, `cpu-used` tuned) + Opus mix
  → `webmmux`. Slow; progress from position/duration queries.
- Include audio off → no audio pad at all (scenario "Export without audio").
- Pause (D5): a pad probe drops buffers while paused and accumulates the paused duration into a
  running-time offset, so the file has no gap and continues in the same muxer.
- Error on the bus (disk full, element error, stream gone with error) → stop as in "Stop
  recording", keep what was written, banner in the Export window (mockup 11). The log names the
  element and error, never content (CLAUDE.md §7).
- Frame rate: the ScreenCast stream may be damage-driven (no frames while nothing changes).
  `videorate` produces a constant 30 fps; whether `pipewiresrc` needs `keepalive-time` or
  `always-copy` is part of probe P7.

### D4 — The control window (Wayland limits stated, not hidden)

A small undecorated toplevel window (OSD pill look) titled "Recording". On Wayland Annota can
neither place nor keep a window above others, so: GNOME places it; the user drags it by its body
or minimises it via GNOME (`Super+H`); it is **in the video** of a screen recording and of a
region recording it overlaps. In a window recording it is not (the stream shows that window
only, measured for stills). The global Record shortcut is therefore the stop path that always
works; the desktop's own screen-sharing indicator may offer another (probe P5). No red frame
around the region while recording: an overlay window could not be kept on top either. The
dashed outline is drawn only during the countdown, whose frames are never recorded.

### D5 — Countdown

The capture countdown OSD (`countdown.rs`, transparent maximised window, empty input region) with
the label "Recording starts in N s". The stream is already open during the countdown; buffers are
dropped until the OSD is gone plus `countdown::SETTLE` (200 ms), then recording starts. 0 s skips
the OSD.

### D6 — Audio

Native .deb: no portal is involved for audio. `pulsesrc` talks to `pipewire-pulse` (default on
Ubuntu 24.04+ and Debian 13). Microphone = default source; system audio = monitor of the default
sink (`@DEFAULT_MONITOR@`). Device presence from `GstDeviceMonitor` (`Audio/Source`); missing →
insensitive row. Mute = `volume mute=true` (silence keeps the track length). Level meters from
`level` messages (setup: a preview pipeline without video and without writing; control: the
recording pipeline). No device picker in this change.

### D7 — Shortcut and command line

GlobalShortcuts ID `record`, description "Record video", preferred trigger `Shift+Ctrl+Print`,
bound together with `region`/`window`/`screen` and shown only as the portal reports it (same
code path as `fix-portal-shortcut-bindings`). Fallback on GNOME 46: custom keybinding running
`annota --record toggle`. Toggle semantics: idle → overlay in Video mode; countdown → cancel;
recording/paused → stop. `--record region|window|screen|toggle` is parsed in `src/cli.rs`
(status 2 on an invalid value, as `--capture`).

### D8 — Settings (GSettings `io.github.trosinde.Annota`)

`record-source` (region|window|screen, default region), `record-microphone` (true),
`record-system-audio` (false), `record-countdown` (0–10, 3), `record-folder` (empty = 
`~/Videos/Annota`), `record-export-format` (mp4|webm, mp4), `record-include-audio` (true),
`shortcut-record` (fallback path). Include pointer reuses the capture setting.

## Dependencies

| What | Provides | Ubuntu 26.04 (measured 2026-11-22, `apt-cache`/`gst-inspect-1.0`) | License | 24.04 / Debian 13 |
|---|---|---|---|---|
| `gstreamer1.0-pipewire` | `pipewiresrc` | already a dependency (ADR 0002) | LGPL-2.1+ | verify |
| `gstreamer1.0-plugins-base` | `videoconvert`, `videoscale`, `videorate`, `audiomixer`, `opusenc`, `volume` | main | LGPL-2.1+ (libopus BSD) | verify |
| `gstreamer1.0-plugins-good` | `pulsesrc`, `level`, `videocrop`, `matroskamux`/`webmmux`, `mp4mux`, `vp9enc` (libvpx) | main; `pulsesrc` ships in it on 26.04 | LGPL-2.1+ (libvpx BSD) | verify whether 24.04 splits `pulsesrc` into `gstreamer1.0-pulseaudio` |
| `gstreamer1.0-plugins-ugly` | `x264enc` | **universe** | plugin LGPL, **libx264 GPL-2+** | verify |
| `gstreamer1.0-plugins-bad` | `voaacenc` (proposed AAC encoder); `openh264enc` as H.264 alternative | **universe** | plugin LGPL, libvo-aacenc Apache-2.0, openh264 BSD-2 | verify, incl. whether 24.04's build has `voaacenc`/`openh264enc` |
| (alternative) `gstreamer1.0-libav` | `avenc_aac` | universe | LGPL (FFmpeg) | — |
| crates | `gstreamer`, `-app`, `-video` (exist), maybe `-pbutils`, `-audio` | — | MIT/Apache-2.0 | MSRV ≤ 1.85, GStreamer ≥ 1.24 API only |

Notes:
- Universe packages are fine as `Depends` for Annota's own APT repository, since universe is
  enabled on a standard Ubuntu desktop; the Debian 13 names are all in main.
- GPL question (CLAUDE.md §10): Annota never links libx264. The plugin is a separate package
  loaded by GStreamer at runtime. The maintainer still has to confirm this (Q2). The fallback is
  `openh264enc`, which is BSD-2 but slower and gives lower quality.
- The `.deb` `Depends` line grows by `gstreamer1.0-plugins-good, gstreamer1.0-plugins-ugly,
  gstreamer1.0-plugins-bad` (plus `gstreamer1.0-pulseaudio` if 24.04 needs it). The scenario
  "Recording elements installed" checks each element on a clean 24.04.

## Portal assumptions that need a probe (CLAUDE.md §5)

No sentence in the spec describes what the portal or GNOME does; the spec states what Annota
sends and how it handles each answer. The behaviour below is **assumed and unmeasured**. Each
item gets a `tools/portal-probe.py` run on GNOME 46 (Ubuntu 24.04) and GNOME 50 (the
maintainer's desktop) in task group 1, before any code relies on it:

| # | Probe | Why it matters |
|---|---|---|
| P1 | `SelectSources` types=MONITOR, multiple=false: is a chooser shown (also with one monitor)? Response codes for close vs. cancel. | waiting indicator, "Source request cancelled" |
| P2 | Monitor stream properties: are `position` and `size` present? Logical or physical px, at scale 1.0 and at 1.25 with `scale-monitor-framebuffer`? | region → stream crop mapping, "Region not on the chosen screen" |
| P3 | Monitor stream buffer size vs. `size` (the window probe saw a 2× buffer with a transparent margin) | crop factors |
| P4 | `AvailableCursorModes` for monitor and window sources; does Embedded draw the pointer into a monitor stream? | "Pointer in the recording" |
| P5 | Does GNOME offer a stop control for a portal ScreenCast session (top-bar indicator / quick settings), and does Annota then receive `Session::Closed`? | "Session ended by the desktop" |
| P6 | Window stream when the window is closed (EOS, error, frozen?) and when it is resized (caps renegotiation, buffer size) | "Recorded window closed", D2 scaling |
| P7 | Frame delivery on a static screen (damage-driven?) over 60 s; does `videorate` get enough buffers, does `pipewiresrc` need `keepalive-time`? | constant frame rate, "Paused time is not recorded" timing |
| P8 | 30 min continuous monitor stream at 4K: dropped frames, memory, CPU with `x264enc superfast` | performance budget |
| P9 | ScreenCast on a GNOME X11 session: is the portal / MONITOR / WINDOW offered? (also `fix-window-capture-screencast` task 2.4) | X11 behaviour, "Source not offered" |
| P10 | Waiting indicator as the chooser's parent for MONITOR (as measured for WINDOW) | D2 |
| P11 | GlobalShortcuts: binding a 4th ID `record` in an existing session — does GNOME show its bind dialog again, which trigger does it report for `Shift+Ctrl+Print`, does GNOME Shell hold that key itself ("Used by GNOME")? | "Record shortcut", "Record row shows the portal's trigger" |
| P12 | Does GNOME's screen-sharing indicator appear in a monitor stream (top bar in the video)? | documentation only |

Non-portal measurements in the same group: `pulsesrc` default source and
`@DEFAULT_MONITOR@` through pipewire-pulse on 24.04; `GstDeviceMonitor` with no microphone.

## Wayland / X11
- Wayland: as above. The control and the countdown are not kept above other windows (D4).
- X11: only through the ScreenCast portal as on Wayland (no X11 grabbing, CLAUDE.md §2).
  Whether GNOME's X11 session offers ScreenCast sources at all is P9; where it does not, the
  sources are insensitive (scenario "Source not offered").

## Risks / open questions
- **Q1 (maintainer):** approve D1 (Export Recording window) or choose alternative A or B.
- **Q2 (maintainer):** depend on `gstreamer1.0-plugins-ugly` (x264, GPL-2+ library loaded at
  runtime) or use `openh264enc` (BSD-2, weaker)?
- **Q3 (maintainer):** AAC through `voaacenc` (plugins-bad) or `avenc_aac` (libav), or Opus
  inside MP4 (no extra package; plays in browsers, VLC and GNOME Videos, not in every player)?
- **Q4 (maintainer):** working file in `~/Videos/Annota` (visible, survives crashes where the
  user can find it) vs. `~/.cache/annota/recordings` (hidden, like autosave).
- **Q5 (maintainer):** is `Shift+Ctrl+Print` the preferred trigger? GNOME may hold it (P11).
- **Q6 (design):** a minimise button on the control (undecorated window), and the countdown ring
  colour (capture Coral vs. destructive red) — see the designer's notes in the mockup index.
- Region mapping (P2/P3) may turn out unreliable under fractional scaling; then D2's rejected
  alternative becomes the plan and the setup flow needs new mockups.
- Live VP9 is avoided by D3, but 4K H.264 at 30 fps on weak CPUs may drop frames (P8). A
  hardware encoder (VA-API) is out of scope for now.
