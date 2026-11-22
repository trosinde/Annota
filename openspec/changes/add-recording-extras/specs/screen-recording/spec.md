# Spec Delta

## ADDED Requirements

### Requirement: Webcam overlay
The recording setup SHALL offer a webcam overlay composited into the recording: shape circle (default), rounded square or square; position in a 3×3 grid (default bottom right); size 10–40 % of the recording's shorter side (default 20 %). The control SHALL offer Swap, which shows the full webcam instead of the screen until swapped back. Without a camera the row is insensitive with "No camera found".

#### Scenario: Webcam in the recording
- **WHEN** the webcam overlay is on with circle, bottom right, 20 %, and the user records a 1000×600 region
- **THEN** every frame shows a 120 px circular webcam picture in the bottom right corner

#### Scenario: Swap
- **WHEN** the user clicks Swap during recording
- **THEN** the recording shows the full webcam until swapped back

#### Scenario: No camera
- **WHEN** no camera exists and the recording setup opens
- **THEN** the Webcam row is insensitive and reads "No camera found"

### Requirement: Cursor effects
The recording setup SHALL offer Highlight pointer, drawing a ring around the pointer into the recording at the pointer position the stream reports. Click animation SHALL be offered only where Annota can observe pointer clicks (design.md, probes E2/E3); elsewhere the option SHALL be absent, never shown without effect.

#### Scenario: Highlight ring
- **WHEN** Highlight pointer is on and the stream reports the pointer at (300, 200)
- **THEN** the recorded frame shows the highlight ring centred on (300, 200)

#### Scenario: Click ripple
- **WHEN** click animation is offered and on, and the user clicks inside the recorded area
- **THEN** the video shows a ripple at the click position

#### Scenario: Clicks not observable
- **WHEN** Annota cannot observe pointer clicks on the current desktop
- **THEN** the recording setup shows no click-animation option

### Requirement: Draw while recording
Users SHALL draw arrows, rectangles and step numbers during recording, composited into the recording; drawings fade after 1, 3 (default), 5 or 10 s or stay until cleared. Where Annota cannot show a drawing surface above the recorded content, Draw SHALL be absent (design.md, probe E4).

#### Scenario: Arrow overlay
- **WHEN** the user draws an arrow during recording
- **THEN** the arrow is visible in the video

#### Scenario: Drawing fades
- **WHEN** fade is set to 3 s and the user draws a rectangle
- **THEN** the rectangle is in the video for 3 s and absent afterwards

### Requirement: Video editor
After a recording, the user SHALL be able to open it in a video editor that trims start and end, cuts out a range, combines clips in a chosen order, and saves the frame at the playhead as a PNG capture. Editing SHALL NOT change the working file until export; every edit can be undone.

#### Scenario: Trim
- **WHEN** the user trims a 20 s recording to 0:02–0:15 and exports it
- **THEN** the exported video lasts 13 s ± 0.1 s and starts with the frame at 0:02

#### Scenario: Cut out a range
- **WHEN** the user cuts out 0:05–0:08 of a 20 s recording and exports it
- **THEN** the exported video lasts 17 s ± 0.1 s

#### Scenario: Combine clips
- **WHEN** the user adds a second 10 s recording after a 20 s recording and exports
- **THEN** the exported video lasts 30 s ± 0.1 s with the second clip last

#### Scenario: Save frame
- **WHEN** the playhead is at 0:04 and the user presses `F`
- **THEN** a PNG capture of the frame at 0:04 is created and delivered like a capture

### Requirement: GIF export
The export SHALL offer GIF with frame rate 5–30 fps (default 15), width (default 800 px, height follows the aspect ratio) and Loop forever (default on). A GIF has no audio.

#### Scenario: GIF
- **WHEN** a 10 s clip is exported as GIF at 15 fps and 800 px
- **THEN** a looping GIF of that size is written
