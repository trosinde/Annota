## MODIFIED Requirements

### Requirement: Window capture
The system SHALL capture a single window, without its shadow. Where the ScreenCast portal offers a window source, the window SHALL be chosen in the desktop's own window chooser for screen sharing. Annota SHALL take exactly one frame of the chosen window, cut it to the window's content, close the screen-sharing session straight away, and hand the image to the after-capture destination. Annota SHALL NOT record, store or keep a stream, and SHALL NOT request or store a restore token, so the chooser appears for every window capture.

The flow, in this order:
1. Without a delay, the chooser opens at once.
2. With a delay set, the chooser opens first and the countdown starts once a window is chosen.
3. The frame is taken when the countdown ends.

While the chooser is open, Annota SHALL show its own small waiting indicator, "Waiting for a window…", with a Cancel button. `Esc` also cancels. The indicator SHALL be the chooser's parent window, so a chooser can be opened even when no other Annota window is open. Cancel or `Esc` ends the attempt and closes the screen-sharing session: no image, no further window, no message. Closing the chooser without choosing is a cancel in the same way, as soon as the desktop reports it. A second window-capture request while the indicator is shown SHALL raise the indicator and start nothing new. If no stream starts, or no frame arrives within 5 seconds of the stream starting, the capture fails with "The capture failed. Nothing was captured." Where the ScreenCast portal offers no window source, window mode SHALL fall back to the Screenshot portal's interactive screenshot UI, and an ending without an image there SHALL say "Nothing was captured". An Annota-drawn click-to-pick overlay for X11 (add-screen-capture mockup 06) is not part of this requirement. It would need direct X11 window enumeration, which the fixed screen-access decision (portal only) rules out.

#### Scenario: Window on Wayland
- **WHEN** the user chooses window mode on GNOME Wayland
- **THEN** GNOME's window chooser for screen sharing is shown, listing the open windows
- **AND** the chosen window, cut to its content and without shadow or transparent margin, is handed to the after-capture destination
- **AND** the screen-sharing session is closed once the frame is taken

#### Scenario: Window mode from the welcome page
- **WHEN** the user activates the Window card on the welcome page and chooses a window in the chooser
- **THEN** a window capture of that window opens in the editor with the toast "Window captured · W × H"
- **AND** GNOME's screenshot UI with its "Selection" mode never appears

#### Scenario: Waiting for the chooser
- **WHEN** the user starts a window capture from a global shortcut with no Annota window open
- **THEN** the indicator "Waiting for a window…" with Cancel appears together with the desktop's window chooser
- **AND** the indicator disappears once a window is chosen

#### Scenario: Cancel while waiting
- **WHEN** the waiting indicator is shown and the user clicks Cancel or presses `Esc`
- **THEN** the screen-sharing session is closed, no image is created and no message is shown
- **AND** a following capture starts normally

#### Scenario: Chooser closed
- **WHEN** the user closes the window chooser without choosing a window and the desktop reports it
- **THEN** the waiting indicator disappears, no image is created and no message is shown

#### Scenario: Delayed window capture
- **WHEN** a delay of 3 s is set and the user starts a window capture
- **THEN** the chooser appears first
- **AND** after a window is chosen, the countdown "Capturing window in 3 s" runs
- **AND** the frame is taken when the countdown ends

#### Scenario: Countdown cancelled
- **WHEN** the user cancels the countdown of a delayed window capture
- **THEN** no image is created and the screen-sharing session is closed

#### Scenario: No frame arrives
- **WHEN** a window was chosen but no frame arrives within 5 seconds
- **THEN** the session is closed and "The capture failed. Nothing was captured." is shown

#### Scenario: No window source
- **WHEN** the ScreenCast portal is missing or does not offer window sources
- **THEN** window mode opens the Screenshot portal's interactive screenshot UI as before

### Requirement: Cursor inclusion
The user SHALL be able to include or exclude the mouse pointer; default is exclude. For region and screen captures Annota draws the pointer into the image itself, since the Screenshot portal has no pointer option for non-interactive captures. For window captures through the ScreenCast portal, Annota SHALL ask the stream to embed the pointer when include-cursor is enabled and the portal offers that, and SHALL ask it to hide the pointer otherwise. Annota draws no pointer into window captures itself. In the Screenshot-portal fallback for window mode, the desktop's screenshot UI decides. A region capture SHALL use the pointer position at the moment the selection is confirmed (`Enter`, or releasing the drag), and SHALL draw the pointer only when its hot spot lies inside the selection.

#### Scenario: Include cursor
- **WHEN** include-cursor is enabled and the user captures the screen
- **THEN** the pointer is visible in the image

#### Scenario: Pointer inside the selection
- **WHEN** include-cursor is enabled, the user has selected a region, moves the pointer inside it and presses `Enter`
- **THEN** the pointer is visible in the image at the position it had when `Enter` was pressed
- **AND** not at the position it had when the overlay opened

#### Scenario: Pointer outside the selection
- **WHEN** include-cursor is enabled and at the moment of confirmation the pointer's hot spot lies outside the selection
- **THEN** no pointer is drawn
- **AND** the captured image is otherwise exactly the selected region

#### Scenario: Pointer on the selection edge
- **WHEN** include-cursor is enabled and the user confirms by releasing the drag, leaving the pointer on the selection's edge
- **THEN** the pointer is drawn only if its hot spot lies inside the selection, so a release at the lower-right corner yields no pointer
- **AND** no part of a pointer is silently cut off at the image border

#### Scenario: Delayed region capture places the pointer
- **WHEN** include-cursor is enabled, the user confirms a region with a delay set and moves the pointer into the region during the countdown
- **THEN** the pointer position is taken after the countdown
- **AND** the pointer is visible in the image where it stood when the countdown ended

#### Scenario: Pointer in a window capture
- **WHEN** include-cursor is enabled, the portal offers an embedded pointer, and the pointer is over the chosen window when the frame is taken
- **THEN** the pointer is visible in the window image

#### Scenario: Cursor excluded
- **WHEN** include-cursor is disabled
- **THEN** no pointer is drawn in any capture mode, and window captures ask the stream to hide it
- **AND** the image is the unmodified portal frame for the captured area

### Requirement: After-capture destination
The system SHALL deliver every capture per the `after-capture` preference: "Copy to Clipboard" puts it on the clipboard; "Open in Editor" opens it in the editor as a new document (add-screen-capture mockup 12); "Both" does both. On X11, "Copy to Clipboard" opens no window, apart from the first-use permission question. On Wayland the desktop accepts a clipboard copy only from an Annota window that has keyboard focus and after Annota has received a key press or click, so "Copy to Clipboard" shows an Annota window to take focus and says in a toast whether the image was copied. A toast SHALL say "Copied to clipboard" only when the image is on the clipboard. Where the copy cannot be made, such as a window or screen capture started from a global shortcut or the command line on Wayland, where Annota has received no key press or click, the toast SHALL offer Copy instead. A new capture is autosaved like any unsaved document (`document-format`) until saved or exported (add-export-share).

#### Scenario: Copy to clipboard
- **WHEN** the after-capture preference is "Copy to Clipboard" on X11 and the user captures a region with the global shortcut while no Annota window is open
- **THEN** the image is on the clipboard
- **AND** no window opens, except the one for the first-use permission question, which is gone before the capture

#### Scenario: Copy to clipboard on Wayland
- **WHEN** the after-capture preference is "Copy to Clipboard" on Wayland and the user captures a region
- **THEN** the image is on the clipboard
- **AND** an Annota window shows the toast "Region captured · W × H · Copied to clipboard"

#### Scenario: Editor destination
- **WHEN** the after-capture preference is "Open in Editor" and the user captures a 400×300 region
- **THEN** the editor window shows a new 400 × 300 document, subtitle `400 × 300 · Not saved`, as the current one
- **AND** a toast "Region captured · 400 × 300" offers Copy

#### Scenario: Both
- **WHEN** the after-capture preference is "Both" and the user captures a region
- **THEN** the capture opens in the editor and the image is on the clipboard

#### Scenario: A copy that cannot be made
- **WHEN** the after-capture preference includes the clipboard and the user starts a window capture with `annota --capture window` on Wayland
- **THEN** the editor's toast "Window captured · W × H" offers Copy and does not say "Copied to clipboard"
- **AND** the clipboard keeps its previous content until Copy is activated, and then holds the image

#### Scenario: Editor unavailable
- **WHEN** the after-capture preference includes "Open in Editor" but the editor window cannot be shown
- **THEN** the capture is saved as PNG into the save folder instead, never overwriting a file, with a toast naming it
