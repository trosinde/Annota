# Add recording extras

**Roadmap phase:** Phase 2 (milestone 0.2–0.4), after `add-screen-recording`.

## Why
The maintainer split screen recording on 2026-11-22: `add-screen-recording` ships the basic
functions (record, countdown, control, audio, MP4/WebM export). This change carries everything
that was moved out, so none of it is lost and each piece can be probed and designed against the
real core pipeline instead of guessed in advance.

## What Changes
- Webcam overlay (picture-in-picture): circle, rounded square or square; position; size; swap
  between screen and full webcam while recording.
- Cursor highlight ring and click animation in the recording.
- Drawing arrows, rectangles and step numbers on screen while recording; drawings fade or stay
  until cleared.
- Video editor after recording: trim start/end, cut out a range, combine clips, save a frame as
  image.
- GIF export (frame rate, width, loop) through gifski.

## Benchmark features covered
- Webcam PiP incl. shape
- Cursor highlight
- Animate clicks
- Draw on screen while recording
- Trim/cut
- Combine clips
- Capture frame
- GIF export

## Out of scope
- Webcam background blur
- Multi-track editing, transitions, titles, zoom-and-pan effects

## Capabilities

### New Capabilities
- none (the `screen-recording` capability is created by `add-screen-recording`)

### Modified Capabilities
- `screen-recording`: new requirements Webcam overlay, Cursor effects, Draw while recording,
  Video editor, GIF export. They are written as ADDED. The video editor also changes what
  happens after stop (core: the Export Recording window opens). That change can only be written
  as a MODIFIED delta once `add-screen-recording` is archived and `openspec/specs/screen-recording`
  exists (CLAUDE.md §4, defect class of 2026-11-10); task 1.1 does it.

## Impact
- Depends on `add-screen-recording` being archived (pipeline, control window, export window).
- New dependency: `gifski` crate (AGPL-3.0-or-later!) — CLAUDE.md §2 names gifski, but its
  license would put a stronger copyleft on the MIT binary (CLAUDE.md §10): needs the
  maintainer's decision before this change is applied (design.md Q1).
- Webcam access through `v4l2src` or PipeWire camera (`pipewiresrc` on a camera node); no portal
  is required for a native .deb, but the Camera portal may matter for Flatpak.
- Several features hit Wayland limits (design.md): a native app cannot observe global clicks,
  cannot keep a drawing overlay above other windows, and gets the pointer only through the
  ScreenCast stream's cursor metadata.
