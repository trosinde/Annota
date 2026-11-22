## MODIFIED Requirements

### Requirement: Window capture
The system SHALL capture a single window without its shadow. Where the ScreenCast portal offers windows, the window SHALL be chosen in the desktop's chooser for screen sharing; Annota SHALL take one frame, cut it to the window, close the session at once, keep no stream, and request no restore token, while a cancellable waiting indicator is the chooser's parent. Otherwise window mode SHALL use the Screenshot portal's interactive UI, where an ending without an image says "Nothing was captured".

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
- **AND** a second window-capture request while the indicator is shown raises it and starts nothing new

#### Scenario: Cancel while waiting
- **WHEN** the waiting indicator is shown and the user clicks Cancel or presses `Esc`
- **THEN** the screen-sharing session is closed, no image is created, no further window opens and no message is shown
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
- **WHEN** a window was chosen but no stream starts, or no frame arrives within 5 seconds of the stream starting
- **THEN** the session is closed and "The capture failed. Nothing was captured." is shown

#### Scenario: No window source
- **WHEN** the ScreenCast portal is missing or does not offer window sources
- **THEN** window mode opens the Screenshot portal's interactive screenshot UI as before

### Requirement: Cursor inclusion
The user SHALL be able to include or exclude the mouse pointer; default is exclude. When it is included, Annota SHALL draw it into region and screen captures itself: for a region at its position on confirming the selection (`Enter` or drag release), and only when its hot spot lies inside the selection. For a ScreenCast window capture Annota SHALL ask the stream to embed the pointer when included and offered by the portal, else to hide it; the Screenshot-portal fallback leaves it to the desktop.

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
- **THEN** Annota draws no pointer in any capture mode, and window captures through the ScreenCast portal ask the stream to hide it
- **AND** the image is the unmodified portal frame for the captured area

### Requirement: After-capture destination
The system SHALL deliver every capture per the `after-capture` preference: "Copy to Clipboard", "Open in Editor" (a new document, add-screen-capture mockup 12) or "Both". On Wayland, a copy needs a focused Annota window that has received a key press or click, so "Copy to Clipboard" shows one. A toast SHALL say "Copied to clipboard" only when the image is on the clipboard, and SHALL offer Copy where the copy could not be made. New captures are autosaved until saved or exported.

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
