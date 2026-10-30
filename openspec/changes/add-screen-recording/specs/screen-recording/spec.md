# Spec Delta

## Purpose
Records the screen with optional audio and webcam and provides basic video editing and export.

## ADDED Requirements

### Requirement: Start and stop
Recording SHALL start after a 3 s countdown (configurable 0–10) for region, window or screen; a floating control shows elapsed time, pause, stop and draw. `Shift+Ctrl+Print` stops recording.

#### Scenario: Stop by hotkey
- **WHEN** a recording runs and the user presses Shift+Ctrl+Print
- **THEN** recording stops and the video opens in the video editor

### Requirement: Audio
Users SHALL record microphone and/or system audio with level meters and mute toggles during recording.

#### Scenario: Mute mic
- **WHEN** the user mutes the mic during recording
- **THEN** the mic track is silent for that segment

### Requirement: Webcam overlay
Users SHALL add a webcam overlay (circle, rounded square, square) with position, size, and switch between screen and full webcam during recording.

#### Scenario: Swap
- **WHEN** the user clicks Swap during recording
- **THEN** the recording shows the full webcam until swapped back

### Requirement: Cursor effects
Recordings SHALL optionally show a cursor highlight ring and a click ripple.

#### Scenario: Click ripple
- **WHEN** click animation is on and the user clicks
- **THEN** the video shows a ripple at the click position

### Requirement: Draw while recording
Users SHALL draw arrows, rectangles and step numbers on screen during recording; drawings fade after a configurable time or stay until cleared.

#### Scenario: Arrow overlay
- **WHEN** the user draws an arrow during recording
- **THEN** the arrow is visible in the video

### Requirement: Video editing and export
The video editor SHALL trim start/end, cut out a range, combine clips, save a frame as image, and export MP4 (H.264), WebM (VP9) and GIF (fps, width, loop).

#### Scenario: GIF
- **WHEN** a 10 s clip is exported as GIF at 15 fps and 800 px
- **THEN** a looping GIF of that size is written
