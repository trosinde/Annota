# Design: Add recording extras

Builds on the approved core `add-screen-recording` (design approved 2026-11-22): its setup
popover, Recording control (with Minimize), red recording countdown, Matroska working file and
the Export Recording window that opens after stop (core D1). Nothing here changes those flows;
it adds rows, buttons, an editor window and a third export format.

## Mockups (design gate)

Mockups live in `docs/design/add-recording-extras/` (designer agent, redrawn 2026-11-22 in the
core's visual language). Scenario-to-mockup mapping, redlines and platform notes:
[`docs/design/add-recording-extras/index.md`](../../../docs/design/add-recording-extras/index.md).
Light 1× renders are linked there; dark and 2× sit next to each file.

Design approved: _pending_

## Maintainer decisions (maintainer, 2026-11-22)

Given to the coordinating session and relayed to this branch.

| # | Question | Decision |
|---|---|---|
| Q1 | GIF encoder (gifski is AGPL-3.0-or-later) | **No gifski.** GIF goes through GStreamer elements already in the dependency set or an MIT/Apache-licensed crate that builds with the project MSRV. Choice: D6. |
| Q2 | Click animation | **Dropped.** The pointer highlight ring stays only if it can be drawn into the video from the stream's cursor metadata (ScreenCast cursor mode metadata), never by an overlay window; needs probe E1/E2. |
| Q3 | Video editor v1 scope | **Trim start/end, cut out a range, save a frame as an image.** Combining clips moves to a later change (proposal "Out of scope"). |
| — | Webcam overlay and drawing | Stay in scope, specified against Wayland limits: no always-on-top windows, no global input. Anything in the video is composited into the stream (D1); every desktop-behaviour assumption is a probe (E1–E7). |

## Context

The core showed that on Wayland Annota can neither place a window nor keep one above others,
and gets no global input. The core therefore has no recording frame and a control window that
GNOME places. This change follows the same rule: what must be in the video is drawn by the
pipeline; what the user must see on the desktop is an ordinary Annota window, and the spec says
where that window ends up in the recording.

## Decisions

### D1 — One compositor in the recording pipeline

```
pipewiresrc ─ videoconvert ─ [videocrop] ─ videoscale ─┐
camera src ─ videoconvert ─ videoscale ─ shape mask ──┼─ compositor ─ cairooverlay ─ videorate ─ x264enc ─ matroskamux
                                                        (screen/camera   (pointer image, ring,
                                                         + swap switch)   strokes, from Annota state)
```
- `compositor` (gst-plugins-base) takes the cropped screen stream as sink 0 and the camera as
  sink 1 (`xpos`/`ypos`/`width`/`height`/`alpha` pad properties). The shape mask is an alpha
  mask applied to the camera frames (circle, rounded square radius 18 %, square).
- Swap (D3) changes pad properties at runtime: screen `alpha=0`, camera scaled to the output and
  letterboxed. No caps renegotiation, no new file.
- `cairooverlay` (gst-plugins-good) draws, per output frame, from shared state guarded by a
  mutex: the pointer image and highlight ring (D4) and the live strokes (D5). It uses the same
  cairo renderer as the editor's annotations for arrows, rectangles and step numbers, so they
  look like Annota annotations (CLAUDE.md §2 "one renderer").
- Every effect is only present in the file. The desktop never shows a camera window, a ring or
  a stroke drawn by the pipeline.
- Unit/pipeline tests use `videotestsrc` for screen and camera and feed synthetic cursor
  positions; frame assertions read pixels at the expected positions.

### D2 — Camera source

- Device presence from `GstDeviceMonitor` (`Video/Source`), as the core does for audio. The
  default camera = the first device the monitor lists; no picker in this change.
- Source element: `v4l2src` on the device path for the native .deb, or `pipewiresrc` on a
  PipeWire camera node — decided by probe E5 (which works on Ubuntu 24.04 without the Camera
  portal, latency, formats, behaviour when another app holds the camera). The Camera portal is
  only relevant for the secondary Flatpak build and is not used here.
- A camera that fails to open or errors mid-recording does **not** stop the recording: the
  camera pad is released, the control shows "Webcam unavailable", the screen continues. The log
  names the error, never content.
- Webcam is offered for all three sources (region, window, screen): the camera is composited
  onto the recorded frame, whose size the pipeline knows.

### D3 — Webcam layout and swap

- Layout in output pixels: side `s = round(size% × min(w, h))`; margin 24 px; 3×3 grid cells
  left/centre/right × top/middle/bottom; centre cells centre the picture on that axis. Example
  (scenario "Webcam placement"): 1000×600, 20 % → 120 px at x = 856, y = 456.
- The camera frame is scaled to cover the shape's square box (centre crop, never stretched).
- Preview in the region overlay: Annota's own overlay window draws a dashed outline of the shape
  at the computed position inside the selection. It is not recorded (the overlay closes before
  the source request, core D2). Window and screen recordings have no preview (their frame is not
  known before the chooser).
- Swap (`W`) is a button of the Recording control, present only with Webcam on. Swapped: camera
  scaled to fit the whole output, letterboxed in black.

### D4 — Pointer highlight from cursor metadata

- Highlight pointer on → `SelectSources` with `cursor_mode` = Metadata (4) when
  `AvailableCursorModes` contains it; the setup row is insensitive otherwise (scenario "Metadata
  cursor not offered"). Include pointer then no longer maps to Embedded: Annota draws the pointer
  image itself from the metadata (or leaves it out when Include pointer is off), so the pointer
  appears exactly once.
- The metadata travels as `SPA_META_Cursor` on PipeWire buffers (position, hotspot, bitmap). It
  is **unmeasured** whether GStreamer's `pipewiresrc` (1.24 on Ubuntu 24.04) exposes it to
  GStreamer at all (probe E1). If it does not, Annota reads the stream with its own consumer on
  the `pipewire` crate (MIT) and pushes buffers into the pipeline through `appsrc`; that is a
  larger change of the core's `screencast.rs` and is decided before task group 3 starts.
- Positions arrive in stream pixels relative to the stream; for region recordings they are
  shifted by the crop offset (core D2 mapping). Whether metadata-only buffers arrive while the
  screen is static but the pointer moves is probe E2 (otherwise the ring would lag on a still
  screen; `videorate` would repeat the last frame with the old position).
- Ring: 44 px diameter at 1× output scale, `--annota-amber`, no fill (exact stroke and opacity in
  the mockup redlines, index.md). No click animation (Q2).

### D5 — Drawing while recording

The hard part. A native app on Wayland cannot draw "on top of the screen" or get global input.
What it can do is open its own window. So:
- **Draw** (`D`) in the Recording control opens a **drawing surface**: an undecorated, transparent
  Annota window made full-screen on the recorded monitor (`gtk_window_fullscreen_on_monitor`,
  monitor chosen from the stream's `position`/`size`, core P2). It takes all pointer input while
  it is open; apps beneath get no clicks. `Esc` or `D` closes it; the recording continues.
- The surface shows the strokes so the user sees what they draw. The pipeline composites the
  same strokes (same model, same timestamps) into the recording through `cairooverlay` (D1).
  For monitor sources the surface itself is also in the stream; both copies coincide when the
  monitor mapping is exact. Probe E3 measures whether they do. Open question O1.
- The surface draws no chrome of its own — anything on it would be in a screen recording. Tool,
  colour, fade and Clear live in the Recording control, which expands while Draw is on, and are
  also on keys (`A`, `R`, `N`, `C`, `Esc`) that work on the surface. Whether GNOME keeps the
  control visible above a full-screen window is probe E3; the keys make it irrelevant.
- Only region and screen recordings: both are monitor streams whose mapping to monitor
  coordinates is known (core P2/P3). For a window stream, the window's position on the monitor
  is unknown on Wayland, so strokes could not be placed. Draw is insensitive there.
- Strokes outside a region's selection are kept in the model but are outside the crop, so they
  are not in the video (scenario "Strokes outside the region").
- Colours: Coral (default), Azure, Amber from the Annota palette. The expanded control has a
  Done pill; Done, the pressed Draw toggle, `D` and `Esc` all turn drawing off.
- Fade: a stroke is opaque for the fade time, then fades out over 0.5 s (hence "absent from
  0:13.5 on" in the scenario). Keep until cleared: until `C`. Step numbers count per recording.
- Pause hides nothing: strokes stay in the model; their fade timer runs on recording time, not
  wall time, so a paused stroke does not fade unseen.

### D6 — GIF export (Q1)

Measured 2026-11-22:
- `gifenc` (gst-plugins-rs) — not packaged in Ubuntu (any release) or Debian
  (packages.ubuntu.com / packages.debian.org: no `gstreamer1.0-plugins-rs`, no `gst-plugins-rs`
  source).
- `avenc_gif` — absent from `gstreamer1.0-libav` 1.28.2 on Ubuntu 26.04 (`gst-inspect-1.0`
  finds only `avdec_gif`); not expected on 24.04 either (task 2.3 verifies). It would also add
  `gstreamer1.0-libav` (FFmpeg, universe) as a dependency.

**Decision:** GStreamer decodes and scales, the **`gif` crate** encodes:
```
filesrc ─ matroskademux ─ decoder ─ [edit list, D8] ─ videorate(fps) ─ videoscale(width) ─ videoconvert ─ appsink (RGBA)
   → gif::Encoder (Repeat::Infinite or no loop extension), Frame::from_rgba_speed (NeuQuant, per-frame palette)
```
- `gif` 0.14.2: MIT OR Apache-2.0, declared `rust-version` 1.62. Dependencies `color_quant`
  1.1.0 (MIT) and `weezl` 0.1.12 (MIT OR Apache-2.0). Resolved with cargo's MSRV-aware resolver
  for `rust-version = 1.91` (the project MSRV is being raised from 1.85 to 1.91 on
  `fix/zip-security-update`, maintainer decision) and built with Rust 1.93 on 2026-11-22; the
  declared MSRVs (1.62 and below) are far under 1.91. The build with the MSRV toolchain is
  verified in task 2.4 (CI). No copyleft (CLAUDE.md §10).
- Quality: per-frame palette, `speed` 10 (tuned in task 5.5 against size and time), frame delay
  `round(100 / fps)` centiseconds. Identical consecutive frames are merged by extending the
  previous frame's delay (smaller files on static screens).
- Runs on a worker thread; progress and Cancel as for WebM (core 08).
- CLAUDE.md §2 says "GIF via gifski". That fixed decision changes: the §2 Video row is updated,
  with an ADR (next free number), after the design approval (task 1.4), not before.

### D7 — Entering the editor and getting its result out

- The Export Recording window (core D1) gains **Edit…** (`Ctrl+E`). It opens the **Edit
  Recording** window (`AdwWindow`, transient for the Export Recording window, which stays open
  and insensitive meanwhile).
- The editor works on an **edit list**: the recording's duration plus a sorted list of kept
  intervals `[(start, end)]`. Trim narrows the first/last interval, Cut Out Range splits one.
  Every change is a `Command` (CLAUDE.md §7) on an undo stack owned by the editor session.
- **Done** hands the edit list to the Export Recording window, which shows the edited duration
  with an "Edited" tag and a poster frame from the first kept frame. **Cancel** drops the
  session's changes (asking "Discard Edits?" if there are any) and returns the previous list.
  Reopening Edit… starts from the current list.
- Export applies the list for every format: MP4 and WebM re-encode the kept intervals (an
  unedited MP4 still remuxes, core D3; an edited one is encoded with `x264enc` because cuts fall
  between keyframes); GIF feeds the same intervals (D6). Durations are frame-exact at 30 fps,
  hence the ± 0.1 s in the scenarios.
- The working file is never written by the editor. The edit list lives in memory with the
  Export Recording window; if Annota quits, the edits are gone and the working file is intact.
- Discard in the Export Recording window deletes the working file as in the core.

### D8 — Editor window

- Window: `AdwWindow` 1100×730, modal to Export Recording, Cancel (start) and Done (end) in the
  header bar instead of a close button (mockups 09–12).
- Preview: `GtkPicture` fed by a playback pipeline (`playbin3` with a `gtk4paintablesink` if
  available on 24.04, else an `appsink` → `GdkMemoryTexture`; decided in task 5.1 by what the
  24.04 packages provide — `gtk4paintablesink` is in gst-plugins-rs and likely missing, probe
  E7). Seeking skips the cut intervals.
- Timeline: thumbnail strip (discoverer/`appsink` frames every N s), one mixed audio waveform,
  start/end handles, playhead, range selection by dragging on the strip. A cut stays visible as a
  hatched band (the ruler keeps recording time; "Result" shows the edited length). Minimum kept
  length 0.5 s.
- Shortcuts (`decisions.md` D5): Play/Pause `Space`, Cut Out Range `Delete`, Save Frame `F`;
  Undo `Ctrl+Z`, Redo `Shift+Ctrl+Z`, Done `Ctrl+Enter`, Cancel `Esc`. Split `S` from D5 is
  not in this change (it belongs with combining clips).
- Save Frame: the decoded frame at the playhead, full recording size, PNG via cairo, file
  `<recording name> – frame M-SS.png` in the recordings folder (` (2)` suffix if taken). Toast
  "Frame saved" with **Open in Annota**, which opens it in the annotation editor like a capture.
  Saved frames are not added to the capture library in this change.

### D9 — Settings (GSettings)

`record-webcam` (false), `record-webcam-shape` (circle|rounded|square, circle),
`record-webcam-position` (0–8, 8 = bottom right), `record-webcam-size` (10–40, 20),
`record-highlight-pointer` (false), `record-draw-tool` (arrow|rectangle|step, arrow),
`record-draw-colour` (0–2, 0 = Coral), `record-draw-fade` (1|3|5|10|0 = keep, 3),
`record-gif-fps` (5–30, 15), `record-gif-width` (160–7680, 800), `record-gif-loop` (true).
`record-export-format` gains `gif`.

## Dependencies

| What | Provides | License | Status |
|---|---|---|---|
| `gstreamer1.0-plugins-base` | `compositor`, `videobox` | LGPL-2.1+ | core dependency; verify elements on 24.04 (task 2.3) |
| `gstreamer1.0-plugins-good` | `v4l2src`, `cairooverlay`, `alpha` | LGPL-2.1+ | core dependency; verify `cairooverlay` is in the 24.04 build |
| `gstreamer1.0-pipewire` | `pipewiresrc` (camera node, alternative to `v4l2src`) | LGPL-2.1+ | core dependency |
| crate `gif` 0.14 + `color_quant`, `weezl` | GIF encoding | MIT OR Apache-2.0 / MIT | new; MSRV 1.91 build in task 2.4 |
| crate `pipewire` (only if E1 says so) | own PipeWire consumer for cursor metadata | MIT | conditional |

No new `.deb` plugin package is expected; task 2.3 confirms or adds one.

## Probes (CLAUDE.md §5)

Every sentence about what GNOME, the portal, PipeWire or a camera does is an assumption until
one of these has been run. Tool: an `extras` group in `tools/recording-probe.py` (task 2.1),
run by the maintainer on GNOME 50 and, where possible, GNOME 46 (Ubuntu 24.04); results in
`probe-results/`. They run after the core probes P1–P12, which this change relies on (P2/P3
mapping, P4 cursor modes, P7 frame delivery).

| # | Probe | Spec / design it decides |
|---|---|---|
| E1 | `AvailableCursorModes` contains Metadata (4) for monitor and window sources on GNOME 46 and 50; does `pipewiresrc` expose `SPA_META_Cursor` (position, hotspot, bitmap) to GStreamer, or is an own `pipewire` consumer needed? | "Pointer highlight", D4 |
| E2 | Cursor metadata: units (stream px vs logical) at scale 1.0 and 1.25; hotspot; bitmap present on every buffer or only on change; do metadata-only buffers arrive when the pointer moves over a static screen, and how often? | "Highlight ring", "Pointer drawn from metadata", D4 |
| E3 | Drawing surface: a transparent undecorated window, `fullscreen_on_monitor` on the recorded monitor during a running ScreenCast session — does it cover that monitor (incl. top bar), does it take all pointer input, does the Recording control stay visible/clickable above it, is it in the monitor stream, and do its pixels line up with strokes composited at the mapped stream position (offset, latency)? Also on GNOME X11. | "Draw while recording", D5, O1 |
| E4 | Keyboard focus on the drawing surface: do `A`/`R`/`N`/`C`/`Esc` reach it right after it opens (focus stealing prevention)? | "Input returns after drawing", D5 |
| E5 | Camera on Ubuntu 24.04 and 26.04: `GstDeviceMonitor` `Video/Source` result, `v4l2src` vs `pipewiresrc` camera node, formats and latency, behaviour when another app holds the camera, whether any permission dialog appears for a native app | "Webcam overlay", "Camera cannot be opened", D2 |
| E6 | Load: 4K monitor stream + 720p camera + `compositor` + `cairooverlay` + `x264enc superfast` for 10 min: dropped frames, CPU (extends core P8) | performance budget |
| E7 | Ubuntu 24.04 package contents: `cairooverlay`, `compositor`, `gtk4paintablesink` (expected missing), `avenc_gif` (expected missing) | D1, D6, D8 |

## Wayland / X11
- Wayland: as above. Annota places nothing; GNOME places the control, the editor and the drawing
  surface (on the requested monitor, E3).
- X11: only through the ScreenCast portal as on Wayland. Metadata cursor and the drawing
  surface are probed on "GNOME on Xorg" too (E1, E3); unavailable features are insensitive with
  the texts in the spec.

## Risks / open questions
- **O1 (maintainer):** in region and screen recordings the drawing surface is itself on the
  recorded monitor, so its strokes are in the stream *and* the pipeline composites the same
  strokes on top. If E3 shows they line up, the result is one stroke. If they do not, the options
  are (a) the surface draws its strokes thinner/at low opacity as a guide only, or (b) for monitor
  sources rely on the surface's own pixels and composite only where the surface is not recorded
  — (b) contradicts the "composited, never a window" rule and needs your decision. Recommendation:
  keep compositing, decide (a) vs. (b) after E3.
- **O2 (maintainer):** Save Frame writes into the recordings folder (`~/Videos/Annota`) and offers
  Open in Annota; it is not added to the capture library (not built yet). Alternative: the
  captures folder. Recommendation: recordings folder until `add-capture-library` exists.
- **O3:** if E1 shows `pipewiresrc` drops cursor metadata, the core's screencast stream code
  changes to an own PipeWire consumer. That is a design change to an approved core decision and
  goes back to the maintainer before task group 3.
- **O4 (designer):** while drawing in a region recording the user cannot see the region (the
  surface has no chrome). Proposal: the surface draws an outline 2 px *outside* the selection,
  which lies outside the crop and so is not in the video. Not drawn yet.
- **O5 (designer):** the 3×3 position cells are 32×24 px, under the 32×32 target rule; 32×32 adds
  24 px to a popover that is already ~670 px tall when expanded.
- **O6 (designer):** editor keys beyond the spec (frame step, Home/End, trim at the playhead) are
  undefined and not drawn.
- If E3 shows the drawing surface cannot cover the recorded monitor or take input reliably, Draw
  is removed from this change (spec and mockups updated, new design gate) rather than shipped
  half-working.
- Edited MP4 exports re-encode and are slower than the core's remux; the progress UI covers it.
