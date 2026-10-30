# Spec Delta

## Purpose
Acquires still images of the screen, a window, a region, the clipboard or a file and hands them to the editor.

## ADDED Requirements

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
The system SHALL register configurable global shortcuts for region (default `Print`), window (default `Alt+Print`) and screen (default `Shift+Print`) capture. With the GlobalShortcuts portal (GNOME ≥ 48) they are bound through the portal while Annota runs. If the portal is unavailable, Annota SHALL offer in Preferences › Capture to create GNOME custom keybindings that run `annota --capture <mode>`.

#### Scenario: Hotkey on GNOME 46
- **WHEN** the GlobalShortcuts portal is unavailable and the user enables the hotkey in preferences
- **THEN** a GNOME custom keybinding for `Print` is created
- **AND** pressing it starts a region capture

### Requirement: Reversible takeover of GNOME's screenshot keys
Per decision D9, the default shortcuts are GNOME's own screenshot keys. On the GNOME-custom-keybinding fallback only (see "Global capture hotkey"), when Annota creates those keybindings the system SHALL move GNOME's bindings for the same keys aside, recorded and never deleted, and SHALL give them back when the Annota shortcut is disabled or changed.

#### Scenario: Takeover is reversible
- **WHEN** Annota's GNOME shortcuts were created and the user disables the region shortcut with Backspace in the Set Shortcut dialog
- **THEN** GNOME's own `Print` binding is restored

#### Scenario: No takeover on the GlobalShortcuts portal path
- **WHEN** the GlobalShortcuts portal is available (GNOME ≥ 48, e.g. Debian 13, Ubuntu 26.04) and Annota registers `Print`/`Alt+Print`/`Shift+Print` as its preferred triggers
- **THEN** GNOME Shell's own bindings for those keys are left untouched, and which of GNOME or Annota actually reacts to the key depends on the compositor

#### Scenario: Shortcut conflict
- **WHEN** the user presses a key combination in the Set Shortcut dialog that another GNOME action already uses (other than GNOME's own screenshot keys, which Annota takes over)
- **THEN** the dialog names that action and "Set" stays insensitive

### Requirement: Cursor inclusion
The user SHALL be able to include or exclude the mouse pointer; default is exclude. For region and screen captures Annota draws the pointer into the image itself, since the Screenshot portal has no pointer option for non-interactive captures; in window mode the desktop's picker decides.

#### Scenario: Include cursor
- **WHEN** include-cursor is enabled and the user captures the screen
- **THEN** the pointer is visible in the image

### Requirement: Image from clipboard or file
The system SHALL create a new capture from an image in the clipboard (`Ctrl+Shift+V` in the main window) or from PNG/JPG/WebP files opened (`Ctrl+O`) or dropped onto Annota. Until the editor exists (`add-annotation-editor`), a new capture is saved as PNG into the save folder and announced with a toast that offers Undo; `add-annotation-editor` turns this into a new editor tab.

#### Scenario: Paste
- **WHEN** the clipboard contains an image and the user presses Ctrl+Shift+V
- **THEN** a new capture with that image is saved as PNG into the save folder and a toast names the file

#### Scenario: Drop an image file
- **WHEN** the user drops a PNG file onto the Annota window
- **THEN** a new capture with that image is saved as PNG into the save folder and a toast names the file

#### Scenario: Unsupported file
- **WHEN** the user drops a file that is not PNG, JPG or WebP
- **THEN** no capture is created and a toast says which formats can be opened

### Requirement: After-capture destination
The system SHALL deliver every region, window and screen capture according to the `after-capture` preference: "Copy to Clipboard" puts the image on the clipboard and opens no window; "Open in Editor" opens it in the editor; "Both" does both. Until `add-annotation-editor` exists, "Open in Editor" instead saves the capture as PNG into the save folder, never overwriting a file, and shows a toast naming it.

#### Scenario: Copy to clipboard
- **WHEN** the after-capture preference is "Copy to Clipboard" and the user captures a region with the global shortcut while no Annota window is open
- **THEN** the image is on the clipboard
- **AND** no window opens

#### Scenario: Editor destination
- **WHEN** the after-capture preference is "Open in Editor" and the user captures a 400×300 region
- **THEN** a 400×300 PNG named by the file-name pattern appears in the save folder
- **AND** a toast "Region captured · 400 × 300" names the saved file

#### Scenario: Both
- **WHEN** the after-capture preference is "Both" and the user captures a region
- **THEN** the PNG is saved into the save folder and the image is on the clipboard

### Requirement: Portal permission handling
If the screenshot portal denies access, the system SHALL show an explanatory dialog with a button that opens the relevant GNOME privacy settings.

#### Scenario: Permission denied
- **WHEN** the portal returns a permission error
- **THEN** the dialog explains how to grant access
- **AND** "Open Settings" opens GNOME Settings at Apps › Annota
