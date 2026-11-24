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
| 15 | [desktop-ended-request](../../../docs/design/add-screen-recording/png/15-desktop-ended-request-light@1x.png) | Toast when the desktop ends the request (Q9, task 4.7) |

Dark and 2× renders sit next to each file (`-dark@1x.png`, `@2x.png`). The out-of-scope screens
of the first set moved to `docs/design/add-recording-extras/`.

Design approved: 2026-11-22 (maintainer)

Decisions recorded with the approval (the maintainer answered them on 2026-11-22; relayed to
this branch by the coordinating session):

| # | Question | Decision |
|---|---|---|
| Q1 | Post-recording flow | **D1**: the Export Recording window opens right after stop. Alternatives A and B rejected. |
| Q2 | H.264 encoder | **x264** (`x264enc`, `gstreamer1.0-plugins-ugly`), libx264 loaded by GStreamer at runtime, never linked. |
| Q3 | AAC encoder | **`voaacenc`** (`gstreamer1.0-plugins-bad`). |
| Q4 | Working-file location | **Visible in `~/Videos/Annota`** (the recordings folder). |
| Q5 | Record shortcut | ~~Preferred trigger `Shift+Ctrl+Print`~~ — superseded by Q7 (2026-11-23). If GNOME holds the key, the row and the Record card show "Used by GNOME" exactly as for the capture shortcuts. |
| Q6 | Control and countdown | The control gets a **Minimize** button (mockups 05, 06); the recording countdown ring is **red** (`--destructive-bg`, mockup 04). |
| Q7 | Preferred Record trigger (2026-11-23, after P11) | **`Super+Alt+R`** (XDG `LOGO+ALT+r`, GTK `<Super><Alt>r`), outside the Print family of Annota's capture defaults. `Shift+Ctrl+Print` had collided with a stored `screen` trigger and GNOME moved `record` to `Ctrl+W`. The UI still shows only the trigger the portal reports. Mockups 05, 06, 12, 13, 14 redrawn. |
| Q8 | Region recording resolution (2026-11-23, after P2/P3) | **Buffer pixel density**: an 800×600 logical selection on a 1.25 monitor is stored as 1000×750; never downscaled to logical size. |
| Q9 | Desktop ends the request during Start (2026-11-23, after the P6 rerun) | Text approved: toast "The desktop ended the screen-sharing request. Nothing was recorded." in the style of mockup 13; its mockup is drawn during apply (task 4.7). |
| D2 | Encoded frame size of a window recording (confirmed 2026-11-23) | **Fixed at start**: the first trimmed window (even-sized) sets the frame size for the whole recording; a later, larger window is scaled down to fit and letterboxed, never stretched. The alternative (frame fixed at the window's monitor size) is rejected. |
| — | Designer assumptions | Accepted as drawn: Pause has the initial focus in the control; Cancel is the default in "Discard Recording?"; an unavailable saved source falls back to Region; Discard/Export sit in the Export Recording footer. |

## Context

The maintainer decided on 2026-11-22 to split video: this change carries the basic functions,
`add-recording-extras` everything else. Recording reuses the capture flows instead of building
parallel ones: the region overlay (`capture` "Region capture"), the ScreenCast window chooser and
its waiting indicator (`fix-window-capture-screencast`, ADR 0002), the countdown OSD
(`capture` "Delayed capture", `src/capture/countdown.rs`), the Include-pointer setting
(`capture` "Cursor inclusion") and the shortcut machinery (`capture` "Global capture hotkey").

## Decisions

### D1 — After stop: the Export Recording window (approved 2026-11-22)

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
  in overlay (logical) coordinates. Measured (P2/P3, GNOME 50, scale 1.25): the stream's
  `position` and `size` are logical (`(0, 0)`, 3072×1728), its buffers physical (3840×2160),
  and the crop meta covers the whole buffer. So: selection − `position`, times the factor
  buffer size ÷ `size` (per axis, from the first buffer), gives the buffer crop; the recording
  keeps the buffer's pixel density (an 800×600 selection at 1.25 records 1000×750; Q8). Not yet
  measured at scale 1.0 or with several monitors. If the chosen monitor does not contain the
  whole selection, the session is closed with a toast (mockup 13).
  Rejected alternative: ScreenCast first and the overlay drawn on the stream's first frame. That
  avoids the coordinate mapping but changes the capture flow and the order of dialogs; revisit if
  P2/P3 show the mapping is unreliable.
- **Window**: `capture`'s window flow (chooser + "Waiting for a window…"), but the stream is kept
  and recorded instead of one frame. Measured (P6, 20:48): the buffer size is fixed for the whole
  stream (6144×3456, 2× the reported `size`) and only the **crop meta** follows the window, on
  resize and maximize alike, with GTK's client-side shadow margin inside it; pixels outside the
  content are undefined (a frame at the maximize moment showed stale white). **The crop meta of a
  window stream is not in buffer pixels** (measured on GNOME 50 by the PR #10 live checks with
  `tools/portal-probe.py --screencast-window`): the buffer is at scale 2, the crop meta at the
  monitor scale (1.25). Content rectangle in the buffer = crop meta × (buffer size ÷ stream
  `size`) ÷ monitor scale, here × 2 / 1.25 = 1.6 (crop (0, 0, 800, 500) → (0, 0, 1280, 800),
  confirmed by the alpha bounds); where the factor cannot be computed, the rectangle is found
  by alpha. This conversion gets its own unit test (scenario "Window crop meta in buffer
  pixels"); the shipped window still capture reads the crop meta as buffer pixels and captures
  only the top-left 62.5 % (fixed separately, not in this change). Every buffer is cropped to
  its own content rectangle, never to the buffer or stream size. The shadow is trimmed by
  `screencast::window_rect`'s alpha rule, computed on the first buffer and again only when the
  crop meta changes (a full alpha scan per 4K buffer is too costly). The encoded frame size is
  fixed at start (the first trimmed window, even-sized for x264); later frames are scaled to fit
  and letterboxed, never stretched (D2 confirmed by the maintainer 2026-11-23). Window
  streams are damage-driven: no buffers while the window is still, so `videorate` repeats the
  last frame (requirement "Recording frame rate").
- **Screen**: monitor source, recorded whole. Annota cannot preselect the monitor under the
  pointer; the desktop's chooser decides.
- No restore token anywhere, as for window stills: the chooser is the user's consent each time.
- Pointer: `cursor_mode` Embedded when Include pointer is on and `AvailableCursorModes` offers
  it, else Hidden (`screencast::cursor_mode`, reused). Measured (P4): Hidden draws no pointer,
  Embedded draws it into the frame. Metadata draws no pointer and adds a
  `GstVideoRegionOfInterestMeta` per buffer (the pointer position) — noted for
  `add-recording-extras` E1, not used here.

### D3 — Pipeline and working file

As built (two pipelines joined by Annota's glue; see "Decided during apply", Phase A and the
review fixes):
```
sources:  pipewiresrc ─ appsink        pulsesrc ─ appsink (default source; default sink .monitor)
                           │ glue: pause, running-time offset, heartbeat, stop
                           ▼
encoder:  appsrc ─ videocrop ─ videoconvert ─ videoscale(add-borders) ─ I420 W×H ─ videorate(30 fps) ─ x264enc ─┐
          appsrc ─ volume(mute) ─ level ─ audioconvert ─ audioresample ─ opusenc (one per source) ──────────────┼─ matroskamux ─ filesink
```
- Working file: Matroska in the recordings folder (`~/Videos/Annota`), one video track and one
  Opus track per enabled audio source. Matroska stays playable when the process dies (scenario
  "Recording survives a crash", verified by a test that records in a child process with live
  test sources and kills it with SIGKILL 10 s in). `x264enc`
  `speed-preset=superfast`, `tune=zerolatency` off, constant quality (`pass=quant`,
  `quantizer≈20`, tuned in task 2.x against CPU load at 4K).
- Export MP4: **remux** the H.264 track unchanged (`matroskademux ! h264parse ! mp4mux`), mix
  the audio tracks (`audiomixer`) and encode AAC → fast, no video quality loss. `mp4mux` with
  `faststart`.
- Export WebM: transcode to VP9 (`vp9enc`, `row-mt`, `deadline=1`, `cpu-used` tuned) + Opus mix
  → `webmmux`. Slow; progress from position/duration queries.
- Include audio off → no audio pad at all (scenario "Export without audio").
- Pause (D5): ~~a pad probe drops buffers while paused~~ (superseded, see "Decided during
  apply": the glue between the two pipelines drops them) and accumulates the paused duration
  into a running-time offset, so the file has no gap and continues in the same muxer.
- Error on the bus (disk full, element error, stream gone with error) → stop as in "Stop
  recording", keep what was written, banner in the Export window (mockup 11). The log names the
  element and error, never content (CLAUDE.md §7).
- Frame rate (settled without a measurement, requirement "Recording frame rate"): the
  pipeline assumes nothing about the stream's rate. Caps say `framerate=0/1` (variable,
  measured P2/P3, `max-framerate=60/1`); the stream may be damage-driven, deliver bursts or
  stop while nothing changes. Frames keep the stream's timestamps, and `videorate` (with a
  repeat of the last frame across gaps) produces the constant 30 fps the encoder needs. A
  `keepalive-time` on `pipewiresrc` is allowed as an optimisation, never relied on. P7 stays
  informational.

### D4 — The control window (Wayland limits stated, not hidden)

A small undecorated toplevel window (OSD pill look) titled "Recording". On Wayland Annota can
neither place nor keep a window above others, so: GNOME places it; the user drags it by its body
or minimises it with its Minimize button (`gtk_window_minimize`) or via GNOME (`Super+H`); it is **in the video** of a screen recording and of a
region recording it overlaps. In a window recording it is not (the stream shows that window
only, measured for stills). The global Record shortcut is therefore the stop path that always
works. GNOME's screen-sharing indicator in GNOME's panel (a screen-cast icon with a small ■ stop
button; on the maintainer's desktop in the bottom panel) offers another: pressing ■ sends
`Session.Closed` (3.27 s after the stream started in the probe), buffers simply stop (last at
3.05 s), and there is no EOS and no error (P5). Annota therefore finishes the recording on
`Session.Closed` and never waits for an EOS. The indicator itself is in the
video of a screen recording (P12, seen in the P4 frames). No red frame
around the region while recording: an overlay window could not be kept on top either. The
dashed outline is drawn only during the countdown, whose frames are never recorded.

### D5 — Countdown

The capture countdown OSD (`countdown.rs`, transparent maximised window, empty input region) with
the label "Recording starts in N s" and a red ring (`--destructive-bg`) instead of the capture's
Coral one (Q6). The stream is already open during the countdown; buffers are
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

GlobalShortcuts ID `record`, description "Record video", preferred trigger `Super+Alt+R` (Q7),
bound together with `region`/`window`/`screen` and shown only as the portal reports it (same
code path as `fix-portal-shortcut-bindings`). Measured (P11): GNOME bound `record` to `Ctrl+W`
when the preferred `Shift+Ctrl+Print` was already a stored trigger of `screen` on that machine,
so the preferred trigger is a request, never a promise; nothing in the UI names it as the
user's key. The maintainer therefore moved the preferred trigger out of the Print family (Q7). Fallback on GNOME 46: custom keybinding for `<Super><Alt>r` running
`annota --record toggle`. Toggle semantics: idle → overlay in Video mode; countdown → cancel;
recording/paused → stop. `--record region|window|screen|toggle` is parsed in `src/cli.rs`
(status 2 on an invalid value, as `--capture`).

### D8 — Settings (GSettings `io.github.trosinde.Annota`)

`record-source` (region|window|screen, default region), `record-microphone` (true),
`record-system-audio` (false), `record-countdown` (0–10, 3), `record-folder` (empty = 
`~/Videos/Annota`), `record-export-format` (mp4|webm, mp4), `record-include-audio` (true),
`record-export-folder` (empty = the recordings folder; added during apply, Phase D1),
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
  loaded by GStreamer at runtime. The maintainer accepted this on 2026-11-22 (Q2); `openh264enc`
  (BSD-2) stays the documented fallback.
- Measured 2026-11-24 in clean containers (`probe-results/2026-11-24-containers-elements.md`):
  every element of both pipelines, including `h264timestamper` and `openh264dec`, comes from
  plugins-base/-good/-ugly/-bad on Ubuntu 24.04, Ubuntu 26.04 and Debian 13; `pulsesrc` is in
  plugins-good on 24.04 as well (no `gstreamer1.0-pulseaudio`). The export's H.264 decoder
  prefers `avdec_h264` when `gstreamer1.0-libav` happens to be installed, else plugins-bad's
  `openh264dec`, so libav is not a dependency. `debian/control` Depends: plugins-good,
  plugins-ugly, plugins-bad (task 5.5).
- The `.deb` `Depends` line grows by `gstreamer1.0-plugins-good, gstreamer1.0-plugins-ugly
  (x264enc), gstreamer1.0-plugins-bad (voaacenc)` (plus `gstreamer1.0-pulseaudio` if 24.04
  ships `pulsesrc` there; `tools/recording-probe.py env` prints the owning package). The scenario
  "Recording elements installed" checks each element on a clean 24.04.

## Portal assumptions that need a probe (CLAUDE.md §5)

Tool: `tools/recording-probe.py` (task 2.1; commands and the dialogs they show in
`manual-checks.md`). Results go to `probe-results/`.

No sentence in the spec describes what the portal or GNOME does; the spec states what Annota
sends and how it handles each answer. Each item gets a probe run on GNOME 46 (Ubuntu 24.04) and
GNOME 50 (the maintainer's desktop) before any code relies on it.

Measured on GNOME Shell 50.1 / xdg-desktop-portal 1.21.1 / xdg-desktop-portal-gnome 50.0 /
PipeWire 1.6.2 / GStreamer 1.28.2, Wayland, one monitor HDMI-4 3072×1728 logical at scale 1.25
(`probe-results/2026-11-23-ubuntu26.04-wayland.md`, runs 17:53 and 18:00). GNOME 46 not run.

| # | Probe | Why it matters | Status (GNOME 50) | Evidence |
|---|---|---|---|---|
| P1 | `SelectSources` types=MONITOR, multiple=false: is a chooser shown (also with one monitor)? Response codes for close vs. cancel. | waiting indicator, "Source request cancelled" | measured | Chooser shown with a single monitor; Start 0 after 4.3–4.8 s. Cancel: Start 1 after 5.0–9.8 s, and the portal has already removed the session (`Session.Close` → UnknownMethod) → scenarios "Chooser cancelled in the desktop", "Waiting indicator with one monitor". Close vs. Esc not told apart. |
| P2 | Monitor stream properties: are `position` and `size` present? Logical or physical px? | region → stream crop mapping, "Region not on the chosen screen" | measured at 1.25 | `position` (0, 0), `size` (3072, 1728) = GDK logical geometry; `mapping_id` absent. Scale 1.0 and multi-monitor not measured. |
| P3 | Monitor stream buffer size vs. `size` | crop factors | measured at 1.25 | Buffer 3840×2160 = `size` × 1.25 (monitor scale); crop meta (0, 0, 3840, 2160) = whole buffer; caps `framerate=0/1`, `max-framerate=60/1`; first frame after 64–73 ms → scenario "Record a region on a scaled monitor". |
| P4 | `AvailableCursorModes` for monitor sources; does Embedded draw the pointer? | "Pointer in the recording" | measured (monitor) | Modes 7. Hidden: no pointer; Embedded: pointer drawn; Metadata: no pointer + `GstVideoRegionOfInterestMeta` (frames looked at, not committed). Window sources not measured. |
| P5 | Does GNOME offer a stop control for a portal ScreenCast session (GNOME's panel / quick settings), and does Annota then receive `Session::Closed`? | "Session ended by the desktop" | measured | Run 18:46: ■ next to the screen-cast icon in the bottom panel pressed → `Session.Closed` after 3.27 s, last buffer at 3.05 s, no EOS, no error. Runs 18:00/18:33: ■ not found, so not pressed; no `Session.Closed`, buffers until 60 s → scenario "Session ended by the desktop" (finish on `Session.Closed`, no EOS wait). |
| P6 | Window stream when the window is closed (EOS, error, frozen?) and when it is resized | "Recorded window closed", "Recorded window resized", D2 | measured | Run 20:48, driven with no human (`tools/recording-probe-drive.py window-end`, 640×400 GTK window): buffer fixed at 6144×3456; crop meta (0, 0, 928, 628) → maximized (0, 0, 3840, 2100) → (0, 0, 928, 628), in monitor-scale px (928×628 = window ×1.25 + CSD shadow; ×1.6 in buffer px, PR #10 live checks); destroying the window → `Session.Closed` 0.06 s later, no EOS, no error, no further buffers; buffers only on damage (3, 1, 0, 1, then 0 per second); content outside the crop undefined. Earlier runs (18:00, 18:35, and two that ended after 3.8 s / 12.6 s with the probe's own terminal chosen) are not reproduced with a normal window; the early `Session.Closed` during Start stays unexplained → scenario "Session closed while waiting". |
| P7 | Frame delivery on a static screen | constant frame rate | invalid run; informational | A video played in both phases (~35 buffers/s with and without keepalive). Settled by design instead (D3, "Recording frame rate"); a rerun is optional. |
| P8 | 30 min continuous monitor stream at 4K: dropped frames, memory, CPU with `x264enc superfast` | performance budget | moved to Verify | Measured with the real implementation (task 6.2), not with the probe. |
| P9 | ScreenCast on a GNOME X11 session (also `fix-window-capture-screencast` task 2.4) | X11 behaviour, "Source not offered" | later | GNOME 50 has no X11 session; needs Ubuntu 24.04 (task 2.1c). |
| P10 | Waiting indicator as the chooser's parent for MONITOR | D2, "Waiting indicator is not recorded" | measured | Start with the probe window's handle as parent answered 0 as for WINDOW. The parent window and its text are visible in the recorded frames → the waiting indicator must be gone before the first recorded frame. `--no-parent` not run. |
| P11 | GlobalShortcuts: binding a 4th ID `record` in an existing session — dialog, reported trigger for `Shift+Ctrl+Print`, "Used by GNOME"? | "Record shortcut", "Portal assigns a different record trigger" | measured | Bind 0 after 17.3 s (dialog shown). `record` → `Ctrl+W`, because `Shift+Ctrl+Print` was already this machine's stored trigger of `screen`; GNOME's keybindings hold none of these keys. `Activated` not seen (key not pressed). |
| P12 | Does GNOME's screen-sharing indicator appear in a monitor stream (GNOME's panel in the video)? | documentation only | measured | GNOME's indicator (screen-cast icon + ■) is visible in all three P4 frames: it is in the video of a screen recording. `monitor --save` not needed. |

Non-portal measurements in the same group: `pulsesrc` default source and
`@DEFAULT_MONITOR@` through pipewire-pulse on 24.04; `GstDeviceMonitor` with no microphone.

## Wayland / X11
- Wayland: as above. The control and the countdown are not kept above other windows (D4).
- X11: only through the ScreenCast portal as on Wayland (no X11 grabbing, CLAUDE.md §2).
  Whether GNOME's X11 session offers ScreenCast sources at all is P9; where it does not, the
  sources are insensitive (scenario "Source not offered").

## Risks / open questions
- Q1-Q9 are decided (see "Design approved" above).
- Encoded size of a window recording: settled, D2 confirmed 2026-11-23 (frame fixed at start,
  larger windows scaled down and letterboxed). Q9's text says only what is true for every
  cause of an early `Session.Closed`, which is unknown (CLAUDE.md §5).
- Region mapping (P2/P3) may turn out unreliable under fractional scaling; then D2's rejected
  alternative becomes the plan and the setup flow needs new mockups (a new design gate).
- Live 4K H.264 at 30 fps on weak CPUs may drop frames (P8). A hardware encoder (VA-API) is out
  of scope for now.
- Pool buffers (P8): the glue holds references to `pipewiresrc` pool buffers: up to two frames
  in each encoder input queue (`QUEUE_BYTES`) and the last frame for repeats. The heartbeat
  replaces the last frame by its own copy as soon as it repeats it (a still screen), so a still
  screen never pins a pool buffer; `videoconvert` to I420 makes new buffers, so x264's lookahead
  holds none. Whether two queued frames can exhaust the pool under load is measured in P8.
- An audio source that delivers nothing at all keeps `matroskamux` waiting (and spinning on one
  core) until stop, with nothing written meanwhile; not seen with `pulsesrc`, which delivers
  silence. To be watched in P8 / task 6.2.
- **Open for the maintainer:** a recorded window that becomes *smaller* than at start is
  currently scaled up to fill the frame, keeping its aspect ratio (D2 only speaks of larger
  windows). The alternative is to show it 1:1, centred, with borders; the reviewer recommends
  1:1.

## Decided during apply

Phase A (display-independent core, `src/recording/`, 2026-11-23):
- **Two pipelines, joined in code.** Sources (`pipewiresrc`, `pulsesrc` → `appsink`) and encoder
  (`appsrc` → … → `matroskamux` → `filesink`) are separate pipelines. The glue between them drops
  buffers while paused and shifts later timestamps by the paused time (D5), and `stop` sends the
  end of stream into the encoder itself, so finishing never depends on the source (P5/P6).
  Pause, resume and mute are kept as timestamped intervals, not as "from now on" flags, so they
  apply to the right buffers whatever is still queued.
- **Still screen:** `videorate` fills a gap with whichever neighbour is closer in time, which
  would show the *next* frame for the second half of a still period. The glue therefore repeats
  the previous frame one frame before a buffer that follows a gap, and up to the stop time on
  stop (requirement "Recording frame rate"). `matroskamux` writes only while every track has
  data, so on a still screen (a damage-driven window or screen stream sends nothing, P6) the
  muxer, the video and with it the audio would stall (and a full audio queue would make
  `pulsesrc` drop sound). A **heartbeat** thread therefore pushes the last frame again at the
  sources' current running time whenever no video buffer came for 200 ms (looked at every
  50 ms): the file keeps advancing in steps of at most ~0.25 s. Glue and heartbeat push under one
  lock, in timestamp order; a buffer the heartbeat overtook is stamped 1 ns after the repeat.
  Tests with synthetic timestamps set the heartbeat's time themselves (`Clock::Manual`).
- **Pause and a still screen:** the newest frame that arrives while paused is kept and pushed at
  the resume time (the screen as it is then); if a new frame follows within half a frame of the
  resume, that one is shown instead. Frames of the paused time are never shown at their time.
- **Region check before anything is written:** the stream's logical `position`/`size` are known
  before the first buffer, so a selection that is not inside it is refused by `start` before the
  working file exists. The buffer crop itself is computed from the first caps.
- **Window frames:** each buffer's content rectangle sets `videocrop` from a pad probe; the frame
  size is fixed by the first rectangle (even-sized), and `videoscale add-borders=true` into fixed
  I420 caps letterboxes later ones (`geometry::letterbox` states the expected placement). A window
  that becomes *smaller* than at start is scaled up to fit as well — D2 only speaks of larger
  windows; open for the maintainer if 1:1 centred is wanted instead (see "Risks / open questions").
- **Crash safety:** `matroskamux` non-streamable (cues and duration written on a clean stop),
  `filesink buffer-mode=unbuffered` (nothing held in a userspace buffer), `x264enc key-int-max=60`
  (at most 2 s per group). The crash test re-runs the test binary as a child process that records
  live `videotestsrc`/`audiotestsrc` through `pipeline::start` and is killed with SIGKILL 10 s in,
  once with a moving picture and once with a picture that stands still after 1 s while the audio
  goes on; each file then plays for at least 9 s, video and audio.
- **Errors:** the first bus error (element name + error text, logged without content) is
  reported as `RecordingEvent::Error`; the glue stops passing data at once and the caller calls
  `stop`. Only an error of the encoder pipeline skips the end of stream (the encoder no longer
  drains); after an error of a source the encoder still gets its end of stream, so x264's
  delayed frames, cues and duration are written.
- **Stop is asynchronous and bounded:** `Recording::stop` closes the glue at once (nothing more
  is written) and returns a future; a worker thread stops the sources, then ends the audio inputs
  first (the muxer may be waiting for an audio track while the video queue is full), pushes the
  last frame up to the stop time, ends the video input and waits for the muxer's end of stream,
  all within `FINISH_TIMEOUT` (10 s); a backstop on the main context delivers `Finished` with
  `RecordingError::Timeout` 1 s later if a state change hangs. The encoder inputs never block:
  the glue waits for room in 5 ms steps and gives up when the recording closes, so stopping the
  sources can never wait on a full queue. `last_video` is read only after the sources stopped.
  `RecordingSession` releases the PipeWire remote and delivers `Ended` once the future resolves.
- **An audio source that never delivered:** `matroskamux` writes nothing until every track has
  its format, which `opusenc` learns only from data; at stop such a track gets 20 ms of silence
  before its end of stream, so the file is finished with an empty track. While such a source
  delivers nothing, `matroskamux` spins and writes nothing (noted for P8 below).
- **Window frames, first buffers:** buffers before the first known content rectangle are dropped
  (otherwise they would pass at the buffer's size and the caps would change mid-stream). A
  rectangle whose even-trimmed size equals the frame size is cropped to exactly that, so the
  first window is not resampled by one pixel.
- **Countdown (D5):** the core has no "drop until armed" state. Phase C either creates the
  `Recording` after the OSD and `SETTLE` are gone (the PipeWire fd stays open meanwhile), or adds
  that state to the glue.
- **Crates:** none added in Phase A (durations in tests are measured by decoding). Task 2.4 stays
  open for `gstreamer-pbutils` (Discoverer) in the export window, which needs a re-vendor.

Phase B (portal session, stop paths, waiting indicator; `src/recording/{source,session,window}.rs`,
`recording::start`, 2026-11-23):
- **Own request flow, not `screencast::choose_window`.** The window still sends `Session.Close`
  after a code-1 answer; the recording spec says the session then counts as closed (P1), so
  `source::request` handles the answers itself and reuses `PortalSession`/`Close`,
  `cursor_mode`, `map_error`, `ParentWindow` and the waiting OSD (made `pub(crate)`, no other
  change to `src/capture`).
- **`Session.Closed` through zbus, not ashpd.** ashpd 0.11.1's `Session::receive_closed`
  deserialises the signal body as `()` and so drops the portal's real `a{sv}` body (fixed in
  ashpd 0.12). Annota subscribes on ashpd's own connection (the portal sends `Closed` to the
  session's owner only) with a plain zbus proxy on the session path, before `SelectSources`.
- **Waiting indicator = the window-capture OSD with a `WaitFor` parameter** (label, icon, title;
  the window title for the screen variant is "Record Screen", read only by assistive technology).
  It is hidden (unmapped) as soon as Start answers; a granted source is handed on only after
  `countdown::SETTLE` (200 ms) more, and the video source is built only after that.
- **Window crop meta and `videocrop`.** `videocrop` applies an upstream `VideoCropMeta` on top
  of its own crop (found with a window stream whose meta does not start at (0, 0); every frame
  was wrong). The content rectangle already accounts for the meta, so Phase A's crop probe now
  removes the meta from the buffer after reading it. The alpha scan of `window.rs` runs on the
  first buffer and when the crop meta or buffer size changes; the monitor scale is the scale of
  the GDK monitor whose logical geometry equals the stream's `size` (and `position`, if given);
  none or several of different scales → the alpha rule alone. Buffers without alpha (BGRx)
  use the converted crop meta, else the whole buffer.
- **Region without `position`:** `(0, 0)` only with a single GDK monitor; otherwise the start
  fails (P2 measured `position` present on GNOME 50).
- **Hooks for Phases C/D and tests** (`recording::Hooks`): video source (`pipewiresrc` on the
  granted fd/node), audio sources, the countdown slot (0 s until 4.3), level events, the start
  outcome (`StartOutcome::message()` gives the toast texts of mockup 13 and Q9) and the end of a
  recording (`Ended`: Phase A's `Finished` + `EndReason`; a stub that logs the path until 5.1).
  `--record region`/`toggle` with nothing running is a marked stub until the setup (4.1) exists;
  the setup calls `recording::start_region(app, selection)`.
- **Stop paths:** `stop()` / `--record toggle` while recording → file finished, PipeWire remote
  released, `Session.Close`; `Session.Closed` while recording → finished at once, nothing sent;
  a pipeline error or the stream's own end of stream → finished with what was written, session
  closed.

Phase C1 (recording setup and countdown, tasks 4.1 and 4.3, plus the Phase B review follow-ups;
`src/recording/setup.rs`, `src/capture/{overlay,countdown,mod}.rs`, 2026-11-23):
- **Video mode enters through the capture flow.** `--record region`, `--record toggle` with
  nothing running (and, from Phase D, the Record shortcut and card) call
  `capture::start_recording_setup`: the same permission handling, frozen Screenshot-portal grab
  and region overlay as a region capture, opened in Video mode. Start Recording ends the overlay
  with `Outcome::Record` (source + selection in logical desktop px); the capture flow hands it to
  `recording::start_from_setup` once it has ended. `Esc` is the overlay's ordinary cancel: no
  ScreenCast call, nothing written. The Annota windows hidden for the grab stay hidden after
  Start Recording (superseded by the Phase C1 review below).
- **The mode pill holds the hint bar.** In Screenshot mode the capture hints sit in the pill
  after the Screenshot | Video toggle; the pill is 24 px above the bottom edge in both modes
  (the capture hint bar was 36 px), so it does not jump when the mode changes.
- **The setup is an overlay child, not a `GtkPopover`.** It is drawn in the overlay window like
  the capture toolbar (kit `.popover` look, the arrow a rotated 14 px square behind its edge), so
  it needs no popup grab inside a full-screen window and is in the window's own screenshots.
  Placement (`overlay::setup_placement`): 18 px right of the selection and top-aligned; else
  left; else below; else above (Phase C1 review); with no room at all at the monitor's right
  edge without an arrow, and then the arrow widget is hidden. It shows on the monitor holding the selection, hides while the selection is dragged,
  and before any selection is centred on every monitor with Start Recording insensitive for
  Region. Start Recording gets the focus once a selection exists.
- **Choices are written as they change** (GSettings bindings; the source toggles and the
  countdown spin by hand), so `Esc` keeps them. A missing device turns its switch off and
  insensitive without writing "off" (the binding is removed first). A saved source that is not
  offered shows Region and is left saved.
- **Level meters** come from a preview pipeline (`<source> ! level ! fakesink`, the setup's own,
  no video, nothing written) that runs only while the setup is shown and only for sources whose
  switch is on; a switched-off source shows an unlit meter at 60 %. Torn down when the overlay
  closes or returns to Screenshot mode. Devices come from `audio::probe_async` (a thread) through
  `Hooks::devices`; a recording probes only when an audio source is wanted, before the countdown.
- **The capture's pointer preview** (arrow and chip) is off in Video mode: a recording embeds the
  live pointer.
- **Countdown (D5):** `countdown::run_for` with `Purpose::Recording` (red ring, "Recording starts
  in N s", hints Enter Start now · Esc Cancel, window title "Recording Countdown"); dropping its
  future takes the OSD away. A region's outline needs the monitor's own coordinates, so that
  window is full-screen on the selection's monitor (maximised otherwise, as for captures).
  Measured nowhere yet: whether GNOME hides its top bar under that transparent full-screen window
  during the countdown (manual check with 6.2). The countdown ends early on `Enter`, `Esc`, the
  Record shortcut / `--record toggle` (`Active::Starting` holds a cancel sender) or the session's
  `Closed` (then `DesktopEnded`, nothing sent back). The `Recording` is created only after the
  countdown future resolved, i.e. after the OSD is gone + `SETTLE`.
- **Review follow-ups:** `source::request` closes its session and hides the indicator when its
  future is dropped (`OpenSession`, `Indicator` guards); GStreamer's background `init` runs once;
  the window crop meta is always converted, with the matching monitor's scale, else the one scale
  all monitors share, else 1 (`geometry::fallback_scale`), never alpha over the whole buffer; the
  waiting indicator of a window recording is titled "Record Window".

Phase C1 review fixes (2026-11-24):
- **Annota stays hidden while a recording starts and runs** (decided by the lead; reversible,
  pending the maintainer's confirmation). The windows the capture flow hid for the frozen
  desktop are not shown again after Start Recording: the user chose the area on a desktop
  without Annota, and the waiting indicator is the chooser's parent. They come back when the
  start ends without a recording (`Esc`/Cancel in the indicator, the chooser cancelled, a
  failure, "region not on the screen" / "desktop ended", whose toast is shown in the main
  window) or once the recording has ended (`Ended`; from Phase D the Export Recording window
  opens there instead). `capture::run_hidden` hands them to `recording::start_from_setup`;
  a request that starts nothing (another flow runs, no region) shows them at once.
- **Placement:** the setup also goes above the selection (arrow on its bottom edge) before the
  no-room fallback, and it never covers the mode pill: beside the selection it moves up or down
  to clear the pill (as long as the arrow still meets the selection's height), below or above
  it moves sideways. Without an arrow position the arrow widget is hidden (GtkOverlay put it at
  the monitor's (0, 0) before).
- **Mode pill at the top:** where the pill (24 px above the bottom edge) would cover one of the
  selection's bottom handles, it moves to 24 px below the top edge (unless it would cover a top
  handle there as well), in both modes.
- **Keys:** in Video mode the arrow keys nudge the selection only while the selection area has
  the keyboard focus; in the setup they move the focus. `Enter` anywhere in the setup is Start
  Recording (a capture-phase key handler; the toggles and switches keep `Space`). Start
  Recording is the window's default widget and the countdown spin has `activates-default`, but
  GTK's spin button (4.22, measured) only commits typed text on the first `Enter` and
  activates the default on a second one, so the handler takes the spin's value (`update`) and
  starts at once. The countdown row is no focus stop of its own.
- **Include pointer:** the capture toolbar's Include Pointer follows the `include-pointer`
  setting while the overlay is open, so the setup's row shows there in Screenshot mode.
- **Level preview:** rebuilt once per change on the next idle (each monitor's switch reports
  the same setting change); its state changes run in order on a thread of their own, so a
  hanging audio server cannot stall the overlay; after a preview error the pipeline is set to
  `Null`.

Phase C2 (Recording control, task 4.4, and the kit icons; `src/recording/control.rs`,
`data/ui/recording-control.blp`, 2026-11-24):
- **The control is a transparent undecorated window around the pill.** GTK draws no shadow for
  an undecorated window, so the pill (403 × 52 with both audio sources, as redlined) sits in a
  window with 20/30/40 px margins that hold its OSD shadow; only the pill takes pointer input
  (input region as for the waiting indicator, `window_wait::input_rect` made `pub(crate)`). A
  `GtkWindowHandle` makes the whole body draggable; the buttons keep their clicks.
- **Shown and closed:** `record` shows it once the `RecordingSession` runs (after the countdown).
  `recording::stop()` (Stop, the Record shortcut) closes it at once; every other end (the desktop,
  an error, the stream's end) closes it on the next 250 ms tick, when the session no longer runs,
  and the delivered `Ended` closes it in any case. ~~A close request (`Alt+F4`) is refused~~
  (superseded in Phase D1: it minimises the control). The control lives exactly as long as its
  recording. A second request (`start` with any mode but
  toggle, `start_region`) calls `control::present()`; only without a control (waiting, countdown)
  the waiting indicator is raised as before.
- **Time:** `Recording::elapsed`, refreshed every 250 ms only while the window is mapped and the
  recording is not paused; the label is only set when its text changes. Found while building
  it: `Timeline::out_time` returned 0 for a moment inside a pause (its `map` counts the pause
  start as paused), so the control would have read `0:00` while paused and a mute set while
  paused would have been stored at 0 s. Fixed in `out_time` (the recorded time before that
  pause), with a unit test; `map` (which buffers are dropped) is unchanged.
- **Keys:** `Space`, `M`, `S` from a key controller on the window in the bubble phase, without
  Ctrl/Alt/Super: `Space` on a focused button activates that button (GTK's own binding), on the
  window body it pauses. `M`/`S` do nothing for a source that is not recorded.
- **Stop's tooltip chip** comes from `Hooks::record_trigger` (a GTK accelerator, `None`: no
  chip), `None` until task 4.5 reports the portal's `record` trigger.
- **Meters:** 5 bars over −60…0 dB, the 5th `--warning-bg` above −3 dB (as the setup's 8-bar
  meter); muted → unlit regardless of the level message. A meter redraws only when its lit bars
  change. While paused no level messages arrive (the `level` element sits behind the glue), so
  the meters keep their last state.
- **Icons:** `annota-record`, `-microphone`, `-speaker` (kit `record`, `microphone`, `speaker`)
  and `-pause`, `-play`, `-stop` (kit glyphs of the same names) are shipped like the other kit
  icons, with the kit's strokes converted to filled outlines (GTK's symbolic recolouring fills
  shapes). The mode pill's Video segment and Start Recording use `annota-record-symbolic`, the
  setup rows and the unmuted control buttons the kit microphone and speaker. Muted and Minimize
  stay Adwaita (`microphone-disabled-symbolic`, `audio-volume-muted-symbolic`,
  `window-minimize-symbolic`) as "Kit gaps" says.
- ~~**Not observable without a window manager**~~ (superseded in Phase D1: the live tests run
  `twm` on the test display and observe both).

Phase D1 (Export Recording window and export pipelines, tasks 5.1-5.4, plus the Phase C2 review
follow-ups; `src/recording/{export,export_window}.rs`, `data/ui/export-recording.blp`,
`data/ui/discard-recording-dialog.blp`, 2026-11-24):
- **Opens for every end.** `Hooks::on_ended`'s default is `export_window::open`: Stop, the Record
  shortcut, the desktop ending the session, the recorded window closed, the stream's end, an
  error (then with the banner of mockup 11). Annota's windows hidden for the recording are
  shown again first (Phase C1 review), then the Export Recording window is presented, so it is
  the newest window. Nothing else changed in the C1 handling of hidden windows.
- **New key `record-export-folder`** (D8 had none for it; `record-folder` is where working files
  go): the folder of the last successful export; empty = the recordings folder. Written when an
  export has been saved, not when a folder is merely chosen. A stored folder that has been
  deleted since is created again by the next export (`files::create_unique`); one that cannot
  be created fails the export with the banner below.
- **MP4 remux needs `h264timestamper`** (plugins-bad, GStreamer ≥ 1.22): Matroska stores no
  decoding timestamps, and with x264's B-frames `mp4mux` wrote a 20 s recording as 44 s without
  it (measured). Chain: `matroskademux ! queue ! h264parse ! h264timestamper ! mp4mux
  faststart=true`; `mp4mux`'s faststart temporary file is written hidden next to the target
  (not in `/tmp`, which may be a small tmpfs) and removed on cancel or error. Audio: each Opus
  track `opusdec ! audioconvert ! audioresample` into one `audiomixer` (48 kHz stereo) →
  `voaacenc` 160 kbit/s.
- **WebM and the poster frame need a software H.264 decoder**, named explicitly: `avdec_h264`
  (gstreamer1.0-libav), else `openh264dec` (gstreamer1.0-plugins-bad). `decodebin` was tried
  first: it picks a hardware decoder where one ranks higher (NVIDIA's on the build host), whose
  threads kept a test process from exiting. Settled for task 5.5: plugins-bad ships `openh264dec`
  on Ubuntu 24.04, 26.04 and Debian 13 (`probe-results/2026-11-24-containers-elements.md`), so
  `Depends: gstreamer1.0-plugins-bad` covers it; `avdec_h264` is used only where libav happens to
  be installed. A unit test forces `openh264dec` for the poster and a WebM export of a working
  file with B-frames (this host has both). VP9: `vp9enc deadline=1 cpu-used=6 row-mt
  end-usage=cq cq-level=32 target-bitrate=8000000` (cap), threads = cores (≤ 16),
  `keyframe-max-dist=120`; Opus mix like MP4's into `opusenc`; `webmmux`. A 20 s 1920 × 1080
  synthetic recording exports in a few seconds on the build host; tuning with real screen
  content belongs to task 6.2.
- **Progress** = the newest video timestamp that reached the muxer ÷ the duration (Discoverer's,
  else the recorded time), polled every 100 ms on the export's worker thread, which also watches
  the bus and the cancel flag. The target is reserved first with `create_new` and a ` (n)`
  suffix (`files::create_unique`, shared with the working file); cancel or an error removes it.
- **A track that is not used** (audio without Include audio, audio for the poster) goes into
  `queue ! fakesink`: without the queue a paused sink blocks the demuxer's thread and the video
  track never prerolls (measured with the poster).
- **Poster frame:** one frame 1 s in (the middle of a shorter recording), decoded at 384 px wide
  and drawn to cover 192 × 120 through a render-node paintable of that intrinsic size (sharp at
  scale 2; a texture as the picture's paintable made the window ask for the texture's size).
  Discoverer gives size, duration and audio tracks; both run in `gio::spawn_blocking`.
- **Width:** a window's own size request includes the client-side decoration's border (560 gave
  550 px of content on X11), and a window that cannot be resized takes its content's natural
  width (the banner's one-line label is wider): the 560 px sit on the content, in an `AdwClamp`.
- **Deviations from the mockups, pending the maintainer:**
  - The extension (`.mp4`) is the File name row's dimmed suffix at the row's end, as in the
    Export Image dialog (add-export-share), not directly after the typed text as drawn in 07:
    `GtkText` cannot show non-editable text inline. The row's edit pencil is hidden (08, 10).
  - ~~"Discard Recording?" is a window of its own~~ (fixed in the D1 review round, below): it
    is inside the Export Recording window as mockup 10 draws it.
  - An export that fails (folder not writable, element missing, pipeline error) has no mockup:
    the window returns to its settings and the banner reads "The recording could not be
    exported. It is kept."; the cause is logged.
- **Closing during an export** asks "Discard Recording?" as well (redline: "before an export has
  finished"); Discard cancels the export (its partial file removed) and deletes the working file.
  After "Saved", Close and the close button just close.
- **Show in Files:** `org.freedesktop.FileManager1.ShowItems([uri], "")` on the session bus
  (async); if the call fails, `gtk::FileLauncher::open_containing_folder`.
- **Control follow-ups (Phase C2 review):** a close request (`Alt+F4`, the window menu, the
  overview) minimises the control instead of being refused (decided by the lead, pending the
  maintainer: nothing is lost, the recording goes on). The 250 ms clock also stops while the
  toplevel is minimised or suspended (`notify::state`) and resumes when that clears; the map-time
  frame-clock and state handlers are disconnected on unmap. Pause/resume and the mutes are
  announced (`gtk_accessible_announce`, "Paused", "Recording", "Microphone muted", "Microphone
  on", "System audio muted", "System audio on"), but not with GTK's stand-in accessibility
  context (no accessibility bus, `GTK_A11Y=none`), whose missing announce implementation crashes
  GTK 4.22 (measured). The time label keeps its text as its accessible name; "Elapsed time" is
  its description.
- **Minimise and raise are observed** in the live tests: they start `twm` (with
  `RandomPlacement`) on the test display. Minimize sets the toplevel's `MINIMIZED` state within
  a second; a second request makes the control the active window over another one. `present()`
  cannot de-iconify a window under twm (no `_NET_ACTIVE_WINDOW`, GTK then only raises and
  focuses), so the test restores the minimised control through the window manager (`xdotool
  windowmap`), as a user would; on GNOME `present()` uses the activation protocols.

Phase D1 review round (2026-11-24; `src/recording/{export,export_window,live_tests}.rs`,
`export/tests.rs`, `data/ui/export-recording.blp`, `data/style.css`):
- **"Discard Recording?" inside the window.** libadwaita 1.6+ presents an `AdwDialog` of a
  window that cannot be resized in a window of its own. The Export Recording window is made
  resizable just for `present()` and not resizable again at once (measured with libadwaita
  1.9.1 / GTK 4.22: the alert is inside, the window stays 560 px wide and cannot be resized; the
  live tests assert all three). libadwaita 1.5 (Ubuntu 24.04) always presents inside; the toggle
  is harmless there. The alert keeps libadwaita's stock destructive response (red label), as
  Annota's other alerts ("Save Changes?", "A file with this name already exists") do; mockup 10
  draws a solid red button. Pending the maintainer if the solid button is wanted everywhere.
- **The failure banner goes when the next export starts** (back to the opening state: hidden,
  or mockup 11's text after an error end), is centred and wraps, and is announced to assistive
  technology (`control::announce`, same guard against GTK's stand-in context). "Discard
  Recording?" still open when an export ends is closed (`force_close`) on "Saved".
- **MP4 audio/video sync.** `voaacenc` starts its output with 1600 samples (33.3 ms at 48 kHz)
  of encoder delay that neither its timestamps nor `mp4mux` (no edit list for priming)
  account for: the sound played 33 ms behind the picture (measured with a flash and a beep at
  the same time through the real recording pipeline; working file and WebM: 0 ms). The MP4's
  video pad now carries an offset of that delay, so `mp4mux` starts the video with an empty
  edit of 33 ms; measured after the fix: 0.1 ms in GStreamer and ffmpeg (the unit test allows
  5 ms). The constant is `voaacenc`'s (measured on GStreamer 1.28); a different encoder or
  version would show in that test.
- **MP4 progress was 100 % at once:** `h264timestamper` moves its timestamps 1000 h on (and its
  segment with them), so the progress probe now converts to stream time. Found by the new
  mid-run MP4 cancel test, which holds the video for a second (a test-only `stall_at`, since a
  remux finishes within the first progress poll) and checks that the partial file and the
  hidden faststart file are gone.
- **Polish:** separator under the header bar (`top-bar-style: raised_border`), the Format toggle
  as the kit's 34 px toggle group (as the setup's source toggle) labelled by its row, Folder
  row activates its button, "Saved" 26 px heavy (`--fs-title-2`), a long folder in "Saved"
  ellipsized in the middle (it widened the window), the File name row's edit pencil hidden
  without keeping its room, Export's German label "Exportieren" (`msgctxt "verb"`).

Phase D2 (Record shortcut, welcome page Record card, toasts, shortcuts window and texts; tasks
4.5-4.7, 5.6; `src/capture/shortcuts.rs`, `src/application.rs`, `src/preferences.rs`,
`data/ui/{window,preferences,help-overlay}.blp`, 2026-11-24):
- **One code path for four shortcuts.** `capture::shortcuts::GlobalShortcut` is `Capture(mode)`
  or `Record`; the functions that took a `CaptureMode` take `impl Into<GlobalShortcut>`, so the
  capture callers are unchanged. `record` is bound in the same `BindShortcuts` as the capture
  IDs, its trigger resolved and marked "Used by GNOME" by the same rules, and its `Activated`
  runs `app.record toggle`, the action `annota --record toggle` from the GNOME keybinding runs.
- **Preferred trigger spelling:** the shared `Accel::to_xdg_trigger` writes `<Super><Alt>r` as
  `ALT+LOGO+r` (Annota's modifier order), not `LOGO+ALT+r` as Q7 wrote it; the XDG shortcuts
  format does not order modifiers. Not probed with GNOME (P11 bound `Shift+Ctrl+Print`): to be
  seen in task 6.2.
- **Keycap order:** keycaps come from `gtk::accelerator_get_label`, which says "Alt+Super+R", so
  the Record card and the row show `Alt` `Super` `R` where mockups 12 and 14 draw `Super` `Alt`
  `R`. Kept: the row's `GtkShortcutLabel` orders the keys the same way, and so does every other
  Annota surface. Pending the maintainer.
- **The Record card runs `app.record region`, not `toggle`:** idle both open the region overlay in
  Video mode; while a recording runs, `toggle` would stop it, `region` presents the control
  (spec "Second recording request"). During a countdown the card does nothing visible.
- **Stop's tooltip chip** = the Record trigger every surface shows (the portal's report, or the
  setting on the GNOME-keybinding path), none when it is text, unbound, or held by GNOME (that
  key does not stop the recording; the launcher's Capture hint follows the same rule).
- **GNOME-keybinding path, existing setups:** where Annota's GNOME shortcuts were created before
  `record` existed, its keybinding (`annota --record toggle` on `shortcut-record`) is added at
  start-up if `custom-keybindings` does not list it and the shortcut is not disabled; otherwise
  the row would claim a keybinding that does not exist. A user who deletes that keybinding in
  GNOME Settings gets it back at the next start; disabling it in Preferences keeps it gone.
- **Toasts (mockups 13, 15):** `Hooks::on_outcome`'s default calls
  `application::show_recording_message`: `present_window` (the main window, created or shown
  again), a kit toast without an action, the focus on the Record card. GTK shows the focus ring
  only after keyboard input, so with a pointer-started flow the focus is there without the ring
  mockup 13 draws.
- **Welcome description** keeps `max-width-chars: 52` but is now `halign: center`: with four cards
  the row is 660 px wide and the label wrapped at the row's width instead of its own (mockup 12
  wraps after "ready" / "to").
- **Shortcuts window:** "Record video" in the Capture group with `<Super><Alt>R`, like the capture
  rows' static defaults there (the window shows no portal triggers; unchanged pattern), and a
  "Recording" group with the control's `Space`, `M`, `S`. The setup's `Enter`/`Esc` are not
  listed, as the region overlay's keys are not.
