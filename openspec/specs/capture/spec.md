# capture Specification

## Purpose
Acquires still images of the screen, a window, a region, the clipboard or a file and hands them to the editor.

## Requirements

### Requirement: Region capture
The system SHALL freeze the current screen image and show a full-screen selection overlay. The overlay SHALL display the selection size in pixels, a magnifier loupe near the pointer, and SHALL snap to window edges when the pointer is within 8 px. Arrow keys SHALL move the selection by 1 px (Shift: 10 px). `Esc` SHALL cancel, `Enter` SHALL confirm. The confirmed image is handed to the after-capture destination (see "After-capture destination").

#### Scenario: Select and confirm
- **WHEN** the user starts a region capture, drags a 400×300 rectangle and presses Enter
- **THEN** a 400×300 image of that area is handed to the after-capture destination

#### Scenario: Cancel
- **WHEN** the user presses Esc in the overlay
- **THEN** no image is created and no window is opened

#### Scenario: Fixed size
- **WHEN** the user types a width and height into the overlay size field
- **THEN** the selection is set to exactly that size

#### Scenario: Snap to a window edge
- **WHEN** the user drags a selection corner to within 8 px of a window edge without holding Alt
- **THEN** the corner jumps onto that edge and a violet guide marks it

#### Scenario: Nudge with the keyboard
- **WHEN** a selection exists and the user presses → and then Shift+↓
- **THEN** the selection moves 1 px right and 10 px down without changing its size

### Requirement: Window capture
The system SHALL capture a single window including its shadow-free frame. The window is chosen through the portal's interactive picker (the desktop's own screenshot UI, which GNOME also provides in X11 sessions). An Annota-drawn click-to-pick overlay for X11 (mockup 06) is not part of this change: it would need direct X11 window enumeration, which the fixed screen-access decision (portal only) rules out without an ADR.

#### Scenario: Window on Wayland
- **WHEN** the user chooses window mode on GNOME Wayland
- **THEN** the portal picker is shown and the chosen window is handed to the after-capture destination

### Requirement: Full-screen capture
The system SHALL capture the monitor under the pointer, or all monitors when the preference `capture-all-monitors` is set.

#### Scenario: Multi-monitor
- **WHEN** two monitors are connected and the user captures the screen with the pointer on monitor 2
- **THEN** only monitor 2 is captured

### Requirement: Delayed capture
The system SHALL support a delay of 1–60 s with a visible countdown that is not part of the capture.

#### Scenario: Countdown
- **WHEN** the user starts a capture with a 5 s delay
- **THEN** a countdown is visible for 5 s
- **AND** the captured image does not contain the countdown

### Requirement: Global capture hotkey
The system SHALL register global shortcuts for region (default `Print`), window (`Alt+Print`) and screen (`Shift+Print`) capture: through the GlobalShortcuts portal (IDs `region`, `window`, `screen`) when it exists, else as GNOME custom keybindings that run `annota --capture <mode>`. On the portal path every surface SHALL show only the triggers the portal reports, read again on bind, on `ShortcutsChanged` and when Preferences or the launcher opens, and editing SHALL be left to GNOME Settings.

#### Scenario: Hotkey on GNOME 46
- **WHEN** the GlobalShortcuts portal is unavailable and the user enables the hotkey in preferences
- **THEN** a GNOME custom keybinding for `Print` is created
- **AND** pressing it starts a region capture

#### Scenario: Portal keeps an earlier trigger
- **WHEN** the GlobalShortcuts portal is available, GNOME has bound `screen` to `Shift+Print`, and Annota's screen-capture setting is `Ctrl+Q`
- **THEN** Preferences › Capture, the launcher's Screen row and the welcome page's Screen card show `Shift+Print`
- **AND** none of them shows `Ctrl+Q`

#### Scenario: Shortcut the portal has not bound
- **WHEN** the GlobalShortcuts portal is available and its answer to the bind contains no trigger for `window`
- **THEN** the Window capture row in Preferences › Capture reads "Not set", and the launcher and the welcome page show no keycap for Window
- **AND** before the portal has answered at all, no surface shows a trigger, and none shows Annota's configured accelerator

#### Scenario: Shortcuts are managed by GNOME
- **WHEN** the GlobalShortcuts portal is available and the user activates the Region capture row in Preferences › Capture
- **THEN** no Set Shortcut dialog opens
- **AND** the group explains that GNOME manages these shortcuts and offers "Open GNOME Settings"

#### Scenario: Open GNOME Settings
- **WHEN** the user clicks "Open GNOME Settings" in Preferences › Capture
- **THEN** GNOME Settings opens at Apps › Annota, which has the "Global Shortcuts" row listing Annota's shortcuts

#### Scenario: Trigger taken by GNOME
- **WHEN** the GlobalShortcuts portal is available, it reports `screen` bound to `Shift+Print`, and GNOME's keybinding settings assign `Shift+Print` to "Take a screenshot"
- **THEN** the Screen capture row in Preferences › Capture shows `Shift+Print` with "Used by GNOME: Take a screenshot"
- **AND** the launcher's Screen row and the welcome page's Screen card mark their `Shift+Print` keycaps as used by GNOME
- **AND** pressing `Shift+Print` runs GNOME's screenshot, not an Annota capture

#### Scenario: Bind answered with an error
- **WHEN** the GlobalShortcuts portal is available and answers Annota's bind with an error
- **THEN** Preferences › Capture reads "GNOME has not reported these shortcuts." and its rows show no trigger
- **AND** the launcher and the welcome page show no keycaps, and none of Annota's configured accelerators
- **AND** when Preferences › Capture opens again and `ListShortcuts` now reports triggers, they are shown

#### Scenario: Trigger changed in GNOME Settings
- **WHEN** Annota is running on the portal path and the user changes Annota's screen-capture shortcut in GNOME Settings to `Super+Shift+S`, then opens Preferences › Capture
- **THEN** the Screen capture row shows `Super+Shift+S`, provided the portal reports that trigger
- **AND** pressing `Super+Shift+S` starts a screen capture

### Requirement: Reversible takeover of GNOME's screenshot keys
On the GNOME-custom-keybinding fallback only, creating Annota's keybindings SHALL move GNOME's bindings for the same keys aside, recorded and never deleted, and give them back when the Annota shortcut is disabled or changed (D9). The Set Shortcut dialog SHALL refuse a combination already used by another GNOME action (except GNOME's screenshot keys), by another Annota capture shortcut, or by an Annota application or window shortcut, since a global binding takes the key from every app.

#### Scenario: Takeover is reversible
- **WHEN** Annota's GNOME shortcuts were created and the user disables the region shortcut with Backspace in the Set Shortcut dialog
- **THEN** GNOME's own `Print` binding is restored

#### Scenario: No takeover on the GlobalShortcuts portal path
- **WHEN** the GlobalShortcuts portal is available (GNOME ≥ 48, e.g. Debian 13, Ubuntu 26.04) and Annota binds its shortcuts
- **THEN** GNOME Shell's own bindings for `Print`, `Alt+Print` and `Shift+Print` are left untouched
- **AND** the triggers Annota shows are the ones the portal reported
- **AND** a reported trigger that is one of those GNOME keys is marked "Used by GNOME" (see "Global capture hotkey")

#### Scenario: Shortcut conflict
- **WHEN** the user presses a key combination in the Set Shortcut dialog that another GNOME action already uses (other than GNOME's own screenshot keys, which Annota takes over)
- **THEN** the dialog names that action and "Set" stays insensitive

#### Scenario: Conflict with an Annota application shortcut
- **WHEN** the user presses `Ctrl+Q` in the Set Shortcut dialog for Screen capture
- **THEN** the dialog reads "Used by Annota: Quit" and "Set" stays insensitive

### Requirement: Cursor inclusion
The user SHALL be able to include or exclude the mouse pointer; default is exclude. For region and screen captures Annota draws the pointer into the image itself, since the Screenshot portal has no pointer option for non-interactive captures; in window mode the desktop's picker decides.

#### Scenario: Include cursor
- **WHEN** include-cursor is enabled and the user captures the screen
- **THEN** the pointer is visible in the image

### Requirement: Image from clipboard or file
The system SHALL create a new capture from an image in the clipboard (`Ctrl+Shift+V`) or from PNG/JPG/WebP/`.annota` files opened (`Ctrl+O`) or dropped onto Annota, and open it in the editor as a new document (add-screen-capture mockup 13). A toast SHALL offer Undo, which closes the new document again. A file that is already open SHALL become the current document instead of being opened twice.

#### Scenario: Paste
- **WHEN** the clipboard contains an image and the user presses Ctrl+Shift+V
- **THEN** the editor shows a new document "Pasted Image" with that image, `W × H · Not saved`, and a toast "New capture from clipboard" with Undo

#### Scenario: Drop an image file
- **WHEN** the user drops a PNG file onto the Annota window
- **THEN** the editor shows a new document with that image, named after the file, and a toast "New capture from file" with Undo

#### Scenario: Unsupported file
- **WHEN** the user drops a file that is not PNG, JPG, WebP or `.annota`
- **THEN** no capture is created and a toast says which formats can be opened

#### Scenario: File already open
- **WHEN** the user opens or drops a file that is already open in the editor
- **THEN** that document becomes the current one instead of a duplicate being created

### Requirement: After-capture destination
The system SHALL deliver every capture per the `after-capture` preference: "Copy to Clipboard" puts it on the clipboard and opens no window, apart from the first-use permission question; "Open in Editor" opens it in the editor as a new document (add-screen-capture mockup 12); "Both" does both. A new capture is autosaved like any unsaved document (`document-format`) until saved or exported (add-export-share).

#### Scenario: Copy to clipboard
- **WHEN** the after-capture preference is "Copy to Clipboard" and the user captures a region with the global shortcut while no Annota window is open
- **THEN** the image is on the clipboard
- **AND** no window opens, except the one for the first-use permission question, which is gone before the capture

#### Scenario: Editor destination
- **WHEN** the after-capture preference is "Open in Editor" and the user captures a 400×300 region
- **THEN** the editor window shows a new 400 × 300 document, subtitle `400 × 300 · Not saved`, as the current one
- **AND** a toast "Region captured · 400 × 300" offers Copy

#### Scenario: Both
- **WHEN** the after-capture preference is "Both" and the user captures a region
- **THEN** the capture opens in the editor and the image is on the clipboard

#### Scenario: Editor unavailable
- **WHEN** the after-capture preference includes "Open in Editor" but the editor window cannot be shown
- **THEN** the capture is saved as PNG into the save folder instead, never overwriting a file, with a toast naming it

### Requirement: Portal permission handling
When the portal's permission store holds no answer for Annota, the system SHALL keep an Annota window on screen as the parent of the first frozen-image request, so GNOME can ask, and SHALL keep that window out of the captured image. Only a stored "no" SHALL count as a denial, with a dialog whose Ask Again removes Annota's own stored answer, and no other app's. A refusal with nothing stored SHALL be reported as GNOME being unable to ask.

#### Scenario: First capture, user allows
- **WHEN** no answer is stored, the user starts a region or screen capture and chooses Allow in GNOME's question
- **THEN** that question's request carries a visible Annota window as its parent
- **AND** the image of that request is discarded unread, and the capture comes from a second request made with Annota's windows hidden

#### Scenario: First capture with no window open
- **WHEN** no answer is stored and `annota --capture region` or a shortcut starts a capture with no Annota window open
- **THEN** a window opens for the question
- **AND** it is gone before the capture, so a capture that opens no window leaves none behind

#### Scenario: A window opened during the question
- **WHEN** the user opens an Annota window while GNOME's question is open, and then allows
- **THEN** that window is hidden for the capture and comes back with its documents
- **AND** only the window opened for the question is gone

#### Scenario: A window closed during the question
- **WHEN** the user closes the question's parent window while GNOME's question is open, and then answers
- **THEN** Annota keeps running and that window does not come back

#### Scenario: Question dismissed
- **WHEN** GNOME's question is closed without an answer
- **THEN** nothing is captured, nothing is shown, and a window opened only for the question is gone

#### Scenario: User chooses Deny
- **WHEN** no answer is stored and the user chooses Deny in GNOME's question
- **THEN** no further request is made and the "Screen Access Denied" dialog is shown

#### Scenario: Permission denied
- **WHEN** the permission store holds "no" for Annota and the user starts a region or screen capture
- **THEN** no request is made and no countdown runs, and the "Screen Access Denied" dialog is shown at once
- **AND** the dialog names no control that does not exist, and Ask Again is its default action with the initial focus

#### Scenario: Denied with only the editor open
- **WHEN** the permission store holds "no" for Annota and only the editor window is open
- **THEN** the dialog is shown over the editor and no other window opens

#### Scenario: Denied dialog already open
- **WHEN** the "Screen Access Denied" dialog is open and the user starts another capture
- **THEN** no second dialog is shown

#### Scenario: Ask Again
- **WHEN** the "Screen Access Denied" dialog is open and the user chooses Ask Again
- **THEN** Annota's own entry is removed from the permission store and no other app's entry changes
- **AND** the dialog closes without a toast and nothing is captured

#### Scenario: Asked again after the reset
- **WHEN** the user chose Ask Again and then starts a region or screen capture
- **THEN** nothing is stored, so GNOME's question appears over an Annota window

#### Scenario: Close keeps the answer
- **WHEN** the "Screen Access Denied" dialog is open and the user chooses Close or presses Esc
- **THEN** the dialog closes and the permission store is unchanged

#### Scenario: The answer cannot be reset
- **WHEN** removing the entry fails
- **THEN** the dialog stays open and shows that the answer could not be reset
- **AND** the error is logged without capture content

#### Scenario: The entry is already gone
- **WHEN** the store answers Ask Again with NotFound
- **THEN** this counts as success and the dialog closes

#### Scenario: Ask Again while it is running
- **WHEN** Ask Again is activated again, or the dialog is closed, while the removal runs
- **THEN** no second removal is sent, the dialog stays open until the store answers, and the UI does not block

#### Scenario: GNOME could not ask
- **WHEN** the portal refuses a request with a window on screen and nothing is stored
- **THEN** no "Screen Access Denied" dialog is shown
- **AND** a toast says that GNOME could not ask, on an Annota window the user can retry from: the editor if it is open

#### Scenario: Permission exists
- **WHEN** the permission store holds "yes" for Annota and the user starts a capture
- **THEN** exactly one request is made, with Annota's windows hidden

#### Scenario: Window mode is not checked
- **WHEN** no answer is stored and the user starts a window capture
- **THEN** the desktop's picker is requested with Annota's windows hidden, without asking first

#### Scenario: Store cannot be read
- **WHEN** Annota cannot read the permission store and the portal refuses the first request
- **THEN** the request is repeated once with a window on screen as its parent
