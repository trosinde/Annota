# Add screen recording

**Roadmap phase:** Phase 2 (milestone 0.2–0.4)

## Why
Short videos and GIFs explain flows better than stills.

## What Changes
- Record region, window or screen via ScreenCast portal + PipeWire
- Microphone and system audio, webcam picture-in-picture
- Cursor highlight, click animation, on-screen drawing while recording
- Trim, cut, combine clips, export MP4/WebM/GIF, grab frame

## Benchmark features covered
- Video capture
- Audio (mic/system)
- Webcam PiP incl. shape
- Cursor highlight
- Animate clicks
- Draw on screen while recording
- Trim/cut
- Combine clips
- GIF export
- Capture frame

## Out of scope
- Webcam background blur (later)
- Multi-track editing

## Capabilities

### New Capabilities
- `screen-recording`

### Modified Capabilities
- none

## Impact
- Depends on the MVP changes being archived first.
- New dependencies: gstreamer-rs, gifski
