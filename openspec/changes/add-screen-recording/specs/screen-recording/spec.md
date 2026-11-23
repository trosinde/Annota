# Spec Delta

## ADDED Requirements

### Requirement: Recording setup
Switching the region overlay's mode pill to Video SHALL show the recording setup next to the selection: source Region, Window or Screen, Microphone and System audio with level meters, Include pointer (the capture setting), Countdown 0–10 s (default 3) and Start Recording (`Enter`). `Esc` cancels. The choices SHALL persist between recordings. The Record shortcut, the welcome page's Record card and `--record` open the overlay in Video mode.

#### Scenario: Switch the overlay to video
- **WHEN** the region overlay is open and the user activates Video in the mode pill
- **THEN** the recording setup is shown with the source, audio, pointer and countdown rows and Start Recording

#### Scenario: Choices are remembered
- **WHEN** the user turns System audio on, sets the countdown to 5 s, cancels, and opens the recording setup again
- **THEN** System audio is on and the countdown reads 5 s

#### Scenario: Cancel the setup
- **WHEN** the recording setup is shown and the user presses `Esc`
- **THEN** the overlay closes, no screen-sharing session is requested and nothing is written

### Requirement: Recording source
Start Recording SHALL request one ScreenCast source without a restore token. Region: a monitor source; only the selection is recorded, mapped to buffer pixels by buffer size ÷ stream `size`. Window: the window chooser flow of `capture` "Window capture", keeping the stream. Screen: a monitor source, recorded whole. While the request is open, a waiting indicator with Cancel (`Esc`) SHALL be shown, also with one monitor. Pointer: embedded when Include pointer is on and offered, else hidden.

#### Scenario: Record a region
- **WHEN** the user selects a 800×600 region, starts recording and the portal answers with a monitor stream containing it whose buffers have the same size as its `size`
- **THEN** the recording's frames are 800×600 and show only that area

#### Scenario: Record a region on a scaled monitor
- **WHEN** the user selects a 800×600 region at (100, 100) and the portal answers with a monitor stream of `size` 3072×1728 whose buffers are 3840×2160
- **THEN** the buffer area (125, 125, 1000, 750) is recorded and the recording's frames are 1000×750

#### Scenario: Region not on the chosen screen
- **WHEN** the portal answers a region recording with a monitor stream that does not contain the whole selection
- **THEN** the session is closed, nothing is written, and "The selected area is not on the screen you chose. Nothing was recorded." is shown

#### Scenario: Record a window
- **WHEN** the user starts a window recording and a window is chosen
- **THEN** the "Waiting for a window…" indicator disappears and the recording shows only that window

#### Scenario: Record the screen
- **WHEN** the user starts a screen recording and the portal answers with a monitor stream
- **THEN** the recording's frames have the stream's full size

#### Scenario: Source request cancelled
- **WHEN** the waiting indicator is shown and the user presses `Esc`
- **THEN** the session is closed, nothing is written and no message is shown

#### Scenario: Chooser cancelled in the desktop
- **WHEN** the portal answers Start with response code 1 (cancelled)
- **THEN** the session counts as already closed: Annota sends no `Session.Close`, writes nothing and shows no message

#### Scenario: Session closed while waiting
- **WHEN** the waiting indicator is shown and the portal reports the session closed before it answers Start
- **THEN** the waiting indicator closes at once without waiting for Start's answer, nothing is written, a later answer to that Start is ignored
- **AND** "The desktop ended the screen-sharing request. Nothing was recorded." is shown

#### Scenario: Waiting indicator with one monitor
- **WHEN** a screen recording is started on a desktop with a single monitor
- **THEN** the waiting indicator is shown until the portal answers Start

#### Scenario: Pointer in the recording
- **WHEN** Include pointer is on and the portal offers an embedded pointer
- **THEN** Annota requests the embedded cursor mode
- **AND** with Include pointer off it requests the hidden cursor mode

#### Scenario: Source not offered
- **WHEN** the ScreenCast portal is missing or does not offer the source type the mode needs
- **THEN** that source is insensitive in the setup, with "Screen sharing for this source is not available on this desktop"

### Requirement: Recording countdown
After the source is granted, the waiting indicator SHALL close and the capture countdown OSD SHALL run with "Recording starts in N s" for the configured time. `Enter` starts at once, `Esc` cancels. No frame from before the waiting indicator and the OSD are gone SHALL be in the recording. A countdown of 0 starts at once.

#### Scenario: Countdown is not recorded
- **WHEN** a recording starts with a 3 s countdown
- **THEN** the countdown OSD is shown for 3 s
- **AND** the recording's first frame is taken after the OSD has gone

#### Scenario: Cancel the countdown
- **WHEN** the user presses `Esc` during the countdown
- **THEN** the session is closed, nothing is written and no message is shown

#### Scenario: No countdown
- **WHEN** the countdown is set to 0 s
- **THEN** recording starts as soon as the source is granted, with no OSD

#### Scenario: Waiting indicator is not recorded
- **WHEN** the portal grants a screen source while the waiting indicator is shown
- **THEN** the waiting indicator is closed before the countdown OSD appears
- **AND** the recording's first frame is taken after the waiting indicator has gone, also with a countdown of 0 s

### Requirement: Recording control
While recording, Annota SHALL show a small "Recording" window: elapsed time without paused time (`M:SS`), Pause/Resume (`Space`), Stop, Microphone mute (`M`), System audio mute (`S`), each with a level meter, and Minimize. GNOME places it; the user can move it. Annota does not keep it above other windows or out of the recorded area. A second recording request SHALL raise it and start nothing.

#### Scenario: Elapsed time
- **WHEN** a recording has run for 12 s
- **THEN** the control shows `0:12`

#### Scenario: Minimize the control
- **WHEN** a recording runs and the user activates Minimize in the control
- **THEN** the control window is minimised and the recording continues

#### Scenario: Second recording request
- **WHEN** a recording runs and the user activates the Record card or runs `annota --record screen`
- **THEN** the control is raised and no second session is requested

### Requirement: Pause and resume
Pause SHALL stop writing video and audio until Resume; the paused time SHALL be absent from the recording, which continues without a gap in the same file. The control SHALL show "Paused", a Resume button and the frozen time.

#### Scenario: Paused time is not recorded
- **WHEN** the user records 10 s, pauses for 5 s and records 10 s more, then stops
- **THEN** the recording lasts 20 s ± 0.2 s and contains no frame from the paused 5 s

#### Scenario: Paused state
- **WHEN** the user presses `Space` in the control while recording
- **THEN** the control shows "Paused", Resume and the time from the moment of pausing, which no longer advances

### Requirement: Audio
The user SHALL be able to record the default microphone, the system audio (monitor of the default output) or both, each optional. Mute SHALL write silence for that source until unmuted; the recording keeps its length. Without a device the row SHALL be insensitive with "No microphone found" or "No audio output found". With neither on, the recording has no audio.

#### Scenario: Mute the microphone
- **WHEN** the user mutes the microphone from 0:05 to 0:08 of a recording with microphone on
- **THEN** the microphone signal is silent from 0:05 to 0:08 and present before and after

#### Scenario: Mute system audio
- **WHEN** the user presses `S` in the control while system audio is recorded
- **THEN** the System audio button shows muted and the system audio signal is silent from then on until unmuted

#### Scenario: No microphone
- **WHEN** no audio input device exists and the recording setup opens
- **THEN** the Microphone row is insensitive and reads "No microphone found"

#### Scenario: No audio selected
- **WHEN** Microphone and System audio are both off and the user records 5 s
- **THEN** the recording has a video track and no audio track

### Requirement: Stop recording
Stop, the Record shortcut, the desktop ending the screen-sharing session and the end of a recorded window's stream SHALL each end the recording, finish the working file and open the Export Recording window. Stopping SHALL close the screen-sharing session. GNOME's own stop reaches Annota only as `Session.Closed`, with no end of stream (design.md P5); Annota SHALL finish the recording on it without waiting for one. Whether closing a recorded window ends its stream is unmeasured (P6).

#### Scenario: Stop by button
- **WHEN** a recording runs and the user activates Stop in the control
- **THEN** the session is closed, the control disappears and the Export Recording window opens for this recording

#### Scenario: Stop by hotkey
- **WHEN** a recording runs or is paused and the user presses the Record shortcut
- **THEN** the recording ends as with Stop

#### Scenario: Session ended by the desktop
- **WHEN** a recording runs and the portal reports the session closed while the stream has sent no end of stream
- **THEN** the recording is finished at once without waiting for an end of stream, what was recorded is kept and the Export Recording window opens

#### Scenario: Recorded window closed
- **WHEN** a window recording runs and the window's stream ends (unmeasured: P6)
- **THEN** what was recorded is kept and the Export Recording window opens

### Requirement: Record shortcut
Annota SHALL register a global shortcut "Record video" (ID `record`, preferred `Shift+Ctrl+Print`) under the rules of `capture` "Global capture hotkey": Preferences › Capture and the Record card show only the trigger the portal reports, which may differ from the preferred one; without the portal, a GNOME custom keybinding running `annota --record toggle`. It SHALL open the overlay in Video mode, cancel a running countdown, or stop a running or paused recording.

#### Scenario: Shortcut opens the setup
- **WHEN** no recording runs and the user presses the Record shortcut
- **THEN** the region overlay opens in Video mode with the recording setup

#### Scenario: Shortcut cancels the countdown
- **WHEN** the recording countdown runs and the user presses the Record shortcut
- **THEN** the countdown ends, the session is closed and nothing is written

#### Scenario: Record shortcut on GNOME 46
- **WHEN** the GlobalShortcuts portal is unavailable and the user enables the Record shortcut in Preferences › Capture
- **THEN** a GNOME custom keybinding for `Shift+Ctrl+Print` running `annota --record toggle` is created

#### Scenario: Record row shows the portal's trigger
- **WHEN** the GlobalShortcuts portal reports `record` bound to `Super+Alt+R`
- **THEN** the "Record video" row in Preferences › Capture and the Record card show `Super+Alt+R`

#### Scenario: Portal assigns a different record trigger
- **WHEN** the GlobalShortcuts portal reports `record` bound to `Ctrl+W` after Annota asked for `Shift+Ctrl+Print`
- **THEN** the "Record video" row and the Record card show `Ctrl+W`
- **AND** neither shows `Shift+Ctrl+Print`

#### Scenario: Record trigger taken by GNOME
- **WHEN** the GlobalShortcuts portal reports `record` bound to `Shift+Ctrl+Print` and GNOME's keybinding settings assign `Shift+Ctrl+Print` to one of GNOME's own actions
- **THEN** the "Record video" row shows `Shift+Ctrl+Print` with "Used by GNOME: <action>", and the Record card marks its keycaps as used by GNOME

### Requirement: Recording on the command line
`annota --record region|window|screen` SHALL open the recording flow in that mode: region opens the overlay in Video mode, window and screen request the source with the saved choices. `annota --record toggle` SHALL act as the Record shortcut. Any other value SHALL exit with status 2 and print the valid values.

#### Scenario: Record a screen from the command line
- **WHEN** no recording runs and the user runs `annota --record screen`
- **THEN** a monitor source is requested with the waiting indicator, and the countdown follows once it is granted

#### Scenario: Invalid record mode
- **WHEN** the user runs `annota --record banana`
- **THEN** the process exits with status 2 and prints `region`, `window`, `screen` and `toggle`

### Requirement: Recording frame rate
The recording SHALL have a constant 30 fps whatever rate the screen-sharing stream delivers: frames are timed by the stream's timestamps and repeated or dropped to 30 fps before encoding, so a still screen (no buffers), a variable rate or a burst of buffers never changes the recording's length or timing.

#### Scenario: Still screen
- **WHEN** a 20 s recording runs and the stream delivers no buffer between 0:05 and 0:15
- **THEN** the recording lasts 20 s ± 0.2 s at 30 fps and shows the last frame from before 0:05 until 0:15

#### Scenario: Burst of buffers
- **WHEN** the stream delivers 60 buffers per second for 5 s of a recording
- **THEN** that part of the recording lasts 5 s ± 0.1 s at 30 fps

### Requirement: Working file
While recording, Annota SHALL write a Matroska working file `Recording YYYY-MM-DD HH-MM-SS.mkv` into the recordings folder (default `~/Videos/Annota`, created if missing), never overwriting a file. It SHALL stay playable up to its last written second if Annota ends unexpectedly. On a write or pipeline error the recording SHALL stop, keep what was written and open the Export Recording window with a banner.

#### Scenario: Recording survives a crash
- **WHEN** the Annota process is killed 10 s into a recording
- **THEN** the working file exists and plays for at least 9 s

#### Scenario: Name already taken
- **WHEN** a recording starts and the recordings folder already has a file of that name
- **THEN** the working file is named with a ` (2)` suffix and the other file is unchanged

#### Scenario: Recording error
- **WHEN** writing the working file fails 10 s into a recording
- **THEN** the session is closed, the Export Recording window opens with "The recording stopped because of an error. What was recorded until then is kept."
- **AND** the cause is logged without recording content

### Requirement: Export after recording
The Export Recording window SHALL show a poster frame, duration and size, File name (default the working file's name), Folder (last used, default the recordings folder), Format MP4 (H.264 + AAC) or WebM (VP9 + Opus), and Include audio, mixing both sources into one track (insensitive without audio). Export (`Enter`) SHALL show progress with Cancel and end with "Saved", Show in Files and Close.

#### Scenario: Export as MP4
- **WHEN** the user exports a 20 s recording with microphone as MP4
- **THEN** an `.mp4` file with H.264 video and one AAC audio track, lasting 20 s ± 0.2 s, is written
- **AND** the window shows "Saved" with Show in Files and Close, and the working file is removed

#### Scenario: Export as WebM
- **WHEN** the user exports a recording as WebM
- **THEN** a `.webm` file with VP9 video and Opus audio is written while a progress bar and Cancel are shown

#### Scenario: Export without audio
- **WHEN** the recording has audio and the user turns Include audio off and exports
- **THEN** the written file has no audio track

#### Scenario: Cancel the export
- **WHEN** the user activates Cancel while an export runs
- **THEN** the partly written file is removed, the working file is kept and the window returns to its settings

#### Scenario: Target name taken
- **WHEN** the chosen folder already has a file with the chosen name and extension
- **THEN** the export is written with a ` (2)` suffix and the existing file is unchanged

#### Scenario: Show in Files
- **WHEN** the window shows "Saved" and the user activates Show in Files
- **THEN** the file manager opens the folder with the exported file selected

### Requirement: Discard a recording
Discard in the Export Recording window, and closing that window before an export has finished, SHALL ask "Discard Recording?" with Cancel and Discard (destructive). Discard SHALL delete the working file; Cancel returns to the window.

#### Scenario: Discard
- **WHEN** the user activates Discard and confirms with Discard
- **THEN** the working file is deleted and the window closes

#### Scenario: Close asks first
- **WHEN** the user closes the Export Recording window before exporting
- **THEN** "Discard Recording?" is shown and the working file still exists until Discard is chosen
