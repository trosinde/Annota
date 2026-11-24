# Spec Delta

## ADDED Requirements

### Requirement: Webcam overlay
The recording setup SHALL offer a Webcam row (default off). When on, the default camera's picture SHALL be composited into every recorded frame by the recording pipeline, never shown as a window on the desktop. The choice SHALL persist between recordings. Without a camera the row SHALL be insensitive with "No camera found"; a camera that cannot be opened stops nothing and is reported in the control.

#### Scenario: Webcam in the recording
- **WHEN** Webcam is on with the default layout and the user records a screen
- **THEN** every recorded frame contains the camera picture and no Annota window shows the camera on the desktop

#### Scenario: No camera
- **WHEN** no camera device exists and the recording setup opens
- **THEN** the Webcam row is insensitive and reads "No camera found"

#### Scenario: Camera cannot be opened
- **WHEN** Webcam is on and opening the camera fails when the recording starts
- **THEN** the recording runs without the webcam picture and the control shows "Webcam unavailable"

### Requirement: Webcam layout
The Webcam row SHALL offer shape Circle (default), Rounded square or Square; position as one of nine cells of a 3×3 grid (default bottom right); and size 10–40 % of the recorded frame's shorter side (default 20 %), with a 24 px margin to the frame edge. The region overlay SHALL preview the placement inside the selection. The layout SHALL persist.

#### Scenario: Webcam placement
- **WHEN** Webcam is on with Circle, bottom right, 20 %, and the user records a 1000×600 region
- **THEN** every frame shows a 120 px circular camera picture whose bounding box ends 24 px from the right and bottom edges

#### Scenario: Square in the top left
- **WHEN** the user sets Square, top left and 40 % and records a 1000×600 region
- **THEN** every frame shows a 240 px square camera picture 24 px from the top and left edges

#### Scenario: Layout preview
- **WHEN** the recording setup is open with Webcam on and the user picks the top right cell
- **THEN** the placement preview inside the selection moves to the top right corner

### Requirement: Swap to webcam
With the webcam overlay on, the Recording control SHALL offer Swap (`W`), which records the full camera picture, scaled to the recorded frame size and letterboxed, instead of the screen until the user swaps back with Show Screen (`W`). The swap is composited by the pipeline; the desktop is unchanged.

#### Scenario: Swap
- **WHEN** a recording with the webcam overlay runs and the user activates Swap at 0:05 and Show Screen at 0:09
- **THEN** the frames from 0:05 to 0:09 show only the camera picture at the recorded frame size, and the frames before and after show the screen with the overlay

#### Scenario: No swap without webcam
- **WHEN** a recording runs with Webcam off
- **THEN** the Recording control has no Swap button

### Requirement: Pointer highlight
The recording setup SHALL offer Highlight pointer (default off). When on, Annota SHALL request the ScreenCast metadata cursor mode and draw into each frame the pointer image (when Include pointer is on) and a ring centred on the pointer position the stream reports. Where the metadata mode is not offered, the row SHALL be insensitive with "Not available on this desktop".

#### Scenario: Highlight ring
- **WHEN** Highlight pointer is on and a frame's cursor metadata reports the pointer at (300, 200) in stream pixels
- **THEN** that recorded frame shows the highlight ring centred on (300, 200)

#### Scenario: Pointer drawn from metadata
- **WHEN** Highlight pointer and Include pointer are both on
- **THEN** Annota requests the metadata cursor mode and each frame shows the pointer image from the metadata at the reported position, once

#### Scenario: Highlight without pointer image
- **WHEN** Highlight pointer is on and Include pointer is off
- **THEN** each frame shows the ring and no pointer image

#### Scenario: Metadata cursor not offered
- **WHEN** the ScreenCast portal's available cursor modes do not include metadata
- **THEN** the Highlight pointer row is insensitive and reads "Not available on this desktop"

### Requirement: Draw while recording
During a region or screen recording, the Recording control SHALL offer Draw (`D`): a transparent full-screen drawing surface on the recorded monitor that takes all pointer input until Draw is off (`Esc` or `D`). For a region it SHALL outline the selection 2 px outside it. Strokes SHALL be composited into the recording. In window recordings Draw SHALL be insensitive with "Drawing is available for region and screen recordings".

#### Scenario: Arrow in the recording
- **WHEN** Draw is on during a screen recording and the user drags an arrow from (100, 100) to (300, 200)
- **THEN** the recorded frames from then on show that arrow at those monitor positions

#### Scenario: Input returns after drawing
- **WHEN** Draw is on and the user presses `Esc`
- **THEN** the drawing surface closes, the recording continues and pointer input reaches the apps again

#### Scenario: Draw in a window recording
- **WHEN** a window recording runs
- **THEN** Draw in the Recording control is insensitive and its tooltip reads "Drawing is available for region and screen recordings"

#### Scenario: Region outline while drawing
- **WHEN** Draw is on during a region recording
- **THEN** the drawing surface shows a 2 px outline just outside the selection
- **AND** no recorded frame contains that outline

#### Scenario: Strokes outside the region
- **WHEN** Draw is on during a region recording and the user draws a rectangle that lies outside the selection
- **THEN** no recorded frame shows that rectangle

### Requirement: Drawing tools and fade
While Draw is on, the user SHALL pick Arrow (`A`, default), Rectangle (`R`) or Step number (`N`, counting 1, 2, 3… per recording), one of three colours (Coral default), and Fade after 1, 3 (default), 5 or 10 s or Keep until cleared. Clear (`C`) SHALL remove every stroke from the following frames. The choices SHALL persist.

#### Scenario: Drawing fades
- **WHEN** fade is 3 s and the user draws a rectangle at 0:10
- **THEN** the rectangle is in the frames from 0:10 to 0:13 and absent from 0:13.5 on

#### Scenario: Keep until cleared
- **WHEN** fade is Keep until cleared, the user draws an arrow at 0:05 and presses `C` at 0:20
- **THEN** the arrow is in the frames from 0:05 to 0:20 and absent afterwards

#### Scenario: Step numbers count up
- **WHEN** the user places three step numbers during one recording
- **THEN** they show 1, 2 and 3 in the order placed

### Requirement: Edit a recording
The Export Recording window SHALL offer Edit… (`Ctrl+E`), which opens the recording in an "Edit Recording" window. Done SHALL return to the Export Recording window with the edits kept, showing the edited duration; Cancel SHALL drop the changes of this editing session, asking "Discard Edits?" if there are any. Editing SHALL never change the working file.

#### Scenario: Open the editor
- **WHEN** the Export Recording window shows a 0:20 recording and the user activates Edit…
- **THEN** the Edit Recording window opens with that recording, the playhead at 0:00 and the whole recording selected for keeping

#### Scenario: Done returns to export
- **WHEN** the user trims the recording to 0:02–0:15 and activates Done
- **THEN** the Export Recording window shows the duration 0:13 and "Edited" next to it

#### Scenario: Working file unchanged
- **WHEN** the user cuts a range, activates Done and then Discard in the Export Recording window
- **THEN** the working file was byte-identical until Discard deleted it

#### Scenario: Cancel asks first
- **WHEN** the user has trimmed the recording in the editor and activates Cancel
- **THEN** "Discard Edits?" is shown, and choosing Discard returns to the Export Recording window with the previous edits

### Requirement: Trim and cut
The editor SHALL show a preview with Play/Pause (`Space`) and a timeline with a playhead. Dragging the start and end handles SHALL trim; selecting a range on the timeline and Cut Out Range (`Delete`) SHALL remove it. Every edit SHALL be undoable (`Ctrl+Z`) and redoable (`Shift+Ctrl+Z`). At least 0.5 s SHALL remain.

#### Scenario: Trim
- **WHEN** the user trims a 20 s recording to 0:02–0:15, activates Done and exports it
- **THEN** the exported video lasts 13 s ± 0.1 s and its first frame is the recording's frame at 0:02

#### Scenario: Cut out a range
- **WHEN** the user selects 0:05–0:08 of a 20 s recording, presses `Delete`, activates Done and exports
- **THEN** the exported video lasts 17 s ± 0.1 s and contains no frame from 0:05 to 0:08

#### Scenario: Undo a cut
- **WHEN** the user cuts out a range and presses `Ctrl+Z`
- **THEN** the timeline shows the full recording again and the duration is as before the cut

#### Scenario: Keep at least half a second
- **WHEN** the user drags the end handle towards the start handle
- **THEN** the handle stops 0.5 s after the start handle

### Requirement: Save a frame
The editor SHALL offer Save Frame (`F`), which writes the frame at the playhead, at the recording's full size, as a PNG `<recording name> – frame M-SS.png` into the recordings folder, never overwriting a file, and shows the toast "Frame saved" with Open in Annota, which opens the PNG in the annotation editor.

#### Scenario: Save frame
- **WHEN** the playhead is at 0:04 of a 1280×800 recording and the user presses `F`
- **THEN** a 1280×800 PNG of the frame at 0:04 is written into the recordings folder and the toast "Frame saved" is shown

#### Scenario: Open a saved frame
- **WHEN** the toast "Frame saved" is shown and the user activates Open in Annota
- **THEN** the PNG opens in the annotation editor

### Requirement: GIF export
The Export Recording window's Format SHALL offer GIF next to MP4 and WebM, applying the edits like them. GIF SHALL offer Frame rate 5–30 fps (default 15), Width 160 px up to the recording's width (default 800 px or the recording's width if smaller; height follows) and Loop forever (default on). Include audio SHALL be insensitive with "GIFs have no sound". The choices SHALL persist.

#### Scenario: GIF
- **WHEN** a 10 s 1600×1000 recording is exported as GIF at 15 fps, 800 px, Loop forever on
- **THEN** an 800×500 `.gif` with 150 frames ± 2 and an infinite-loop extension is written

#### Scenario: GIF plays once
- **WHEN** a recording is exported as GIF with Loop forever off
- **THEN** the written GIF has no loop extension

#### Scenario: GIF of an edited recording
- **WHEN** a 20 s recording trimmed to 0:02–0:15 is exported as GIF at 10 fps
- **THEN** the GIF has 130 frames ± 2

#### Scenario: No sound in a GIF
- **WHEN** the user selects GIF in the Export Recording window
- **THEN** Include audio is insensitive, off and reads "GIFs have no sound"
