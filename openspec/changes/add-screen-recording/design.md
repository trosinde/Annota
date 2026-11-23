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
