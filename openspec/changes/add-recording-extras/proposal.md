# Add recording extras

**Roadmap phase:** Phase 2 (milestone 0.2–0.4), after `add-screen-recording`.

## Why
The maintainer split screen recording on 2026-11-22: `add-screen-recording` ships the basic
functions (record, countdown, control, audio, MP4/WebM export). This change carries the extras
that build on that pipeline. Scope was narrowed by the maintainer on 2026-11-22 (design.md
Q1–Q3): no click animation, no combining of clips, no gifski.

## What Changes
- **Webcam overlay**, composited into the video by the pipeline: shape (circle, rounded square,
  square), position (3×3 grid), size (10–40 %), a placement preview in the region overlay, and
  **Swap** to the full camera picture during recording.
- **Pointer highlight**: a ring drawn into each frame from the ScreenCast stream's cursor
  metadata. Offered only where the portal offers the metadata cursor mode.
- **Draw while recording** (region and screen recordings): Draw in the Recording control opens a
  transparent full-screen drawing surface that takes the pointer while it is on; arrows,
  rectangles and step numbers are composited into the video and fade after 1/3/5/10 s or stay
  until cleared.
- **Video editor v1**, entered with **Edit…** from the core's Export Recording window: trim start
  and end, cut out a range, undo/redo, save the frame at the playhead as a PNG. Done returns to
  the Export Recording window; every format exports the edited result.
- **GIF export** as a third format in the Export Recording window (frame rate, width, loop),
  encoded with the `gif` crate (MIT OR Apache-2.0) from frames GStreamer decodes. No gifski.

## Benchmark features covered
- Webcam picture-in-picture incl. shape, position, size, swap
- Cursor highlight
- Draw on screen while recording
- Trim / cut out a range
- Capture a frame from a video
- GIF export

## Out of scope
- **Click animation** — dropped by the maintainer (2026-11-22, Q2): a native app cannot observe
  global clicks on GNOME Wayland.
- **Combining clips** (adding a second recording to the timeline) — moved to a later change
  (2026-11-22, Q3), as is the editor's Split (`S`) from `decisions.md` D5, which only matters
  together with combining.
- Drawing in window recordings (the window's position on the monitor is unknown on Wayland).
- Webcam background blur, a camera picker (the default camera is used), audio in GIFs.
- Multi-track editing, transitions, titles, zoom-and-pan effects, speed changes.
- Recordings and saved frames in the capture library.

## Capabilities

### New Capabilities
- none (the `screen-recording` capability is created by `add-screen-recording`)

### Modified Capabilities
- `screen-recording`: ADDED requirements Webcam overlay, Webcam layout, Swap to webcam, Pointer
  highlight, Draw while recording, Drawing tools and fade, Edit a recording, Trim and cut, Save a
  frame, GIF export. Edit… and GIF change the core's Export Recording window and the setup gains
  rows. These are written as ADDED requirements because `openspec/specs/screen-recording` does
  not exist yet; once `add-screen-recording` is archived, task 1.6 rewrites the parts that change
  core requirements ("Recording setup", "Recording control", "Export after recording") as
  MODIFIED deltas (CLAUDE.md §4, defect class of 2026-11-10).

## Impact
- Depends on `add-screen-recording` being archived (pipeline, control window, Export Recording
  window, probes P1–P12, in particular P2/P3 for the region mapping used by drawing).
- Code: `src/recording/` gains `compositor.rs` (webcam, pointer ring, strokes), `camera.rs`,
  `draw_surface.rs`, `editor/` (Edit Recording window, edit list, timeline), `gif.rs`; changes to
  the setup popover, the Recording control and the Export Recording window; GSettings keys.
- Crates: `gif` 0.14 (MIT OR Apache-2.0, declared MSRV 1.62) with `color_quant` (MIT) and `weezl`
  (MIT OR Apache-2.0). Possibly `pipewire` (MIT) if `pipewiresrc` does not pass cursor metadata
  on (probe E1).
- Packaging: no new plugin package expected — `compositor`, `v4l2src`, `videobox`/`alpha` are in
  `gstreamer1.0-plugins-base`/`-good`, already dependencies of the core (verify, task 2.3).
- Fixed decision in CLAUDE.md §2 ("GIF via gifski") changes: an ADR (next free number) and the §2 row are
  updated in task 1.4 once the design is approved.
- Privacy: camera pictures and drawings exist only in the recording; saved frames go into the
  recordings folder. Nothing leaves the machine.
