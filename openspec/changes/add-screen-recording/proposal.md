# Add screen recording (core)

**Roadmap phase:** Phase 2 (milestone 0.2–0.4). First of two recording changes; the extras
(webcam, cursor effects, drawing, video editor, GIF) follow in `add-recording-extras`.

## Why
Short videos explain flows better than stills. The maintainer decided on 2026-11-22 to ship the
basic functions first ("Grundfunktionen") and to move everything else into a follow-up change,
so that the recording pipeline can be built, probed and released on its own.

## What Changes
- Record a region, a window or a screen through the ScreenCast portal + PipeWire, reusing the
  capture flows: the region overlay (with a Screenshot/Video mode pill), the ScreenCast window
  chooser and its waiting indicator, the capture countdown OSD and the Include-pointer setting.
- Recording setup on the region overlay: source, microphone, system audio, pointer, countdown
  (0–10 s, default 3).
- A small "Recording" control window: elapsed time, pause/resume, stop, microphone and
  system-audio mute with level meters.
- A global "Record video" shortcut (ID `record`, preferred `Super+Alt+R`) that opens the
  setup and stops a running recording; `annota --record region|window|screen|toggle`.
- A crash-safe Matroska working file in the recordings folder (default `~/Videos/Annota`).
- After stop, the Export Recording window: MP4 (H.264 + AAC) or WebM (VP9 + Opus), Include
  audio, progress, Show in Files, Discard. **This post-recording flow needs the maintainer's
  approval** (design.md D1).
- A Record card on the welcome page (`app-shell` "Welcome page").
- Package dependencies on the GStreamer plugins the pipelines use (`debian-packaging`).

## Benchmark features covered
- Video capture (region, window, screen)
- Audio (microphone / system)
- MP4 and WebM export

## Out of scope
Moved to `add-recording-extras` (depends on this change):
- Webcam overlay / picture-in-picture, shapes, swap to full webcam
- Cursor highlight and click animation
- Drawing on screen while recording
- Video editor: trim, cut, combine clips, save a frame
- GIF export (gifski)

Not planned in either change: webcam background blur, multi-track editing, an audio device
picker (the default devices are used), recording several monitors at once, a restore token
that skips the desktop's chooser, recordings in the capture library, a still capture while a
recording runs.

## Capabilities

### New Capabilities
- `screen-recording`: setup, source, countdown, control, pause, audio, stop, shortcut, command
  line, working file, export, discard.

### Modified Capabilities
- `app-shell`: "Welcome page" gains the Record card.
- `debian-packaging`: new requirement "Recording runtime dependencies". It is ADDED rather than
  a MODIFIED "Runtime dependencies", which `fix-window-capture-screencast` already modifies.

## Impact
- Depends on `fix-window-capture-screencast` (ScreenCast session code, waiting indicator,
  ADR 0002) being archived first, and on the MVP changes.
- Code: new `src/recording/` (session, pipeline, control window, export window), changes in
  `src/capture/overlay.rs` (mode pill, setup), `src/capture/screencast.rs` (keep a stream
  instead of one frame), `src/capture/countdown.rs` (label), `src/capture/shortcuts.rs`
  (`record` ID), `src/cli.rs`, the welcome page, Preferences › Capture, GSettings schema.
- Crates: `gstreamer`, `gstreamer-app`, `gstreamer-video` are already added by
  `fix-window-capture-screencast`; `gstreamer-pbutils` (discoverer, for the poster frame and
  duration) and `gstreamer-audio` may be added — MIT/Apache-2.0, MSRV to be checked against
  1.85. gifski moves to `add-recording-extras`.
- Packaging: new runtime GStreamer plugin packages, see design.md "Dependencies"; one of them
  (`gstreamer1.0-plugins-ugly`, x264) carries GPL-2+ code loaded at runtime — open question Q2.
- Privacy: recordings are written only into the recordings folder and the export folder the
  user chooses. Nothing leaves the machine.
