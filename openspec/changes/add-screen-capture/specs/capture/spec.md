# Spec Delta

## Purpose
Acquires still images of the screen, a window, a region, the clipboard or a file and hands them to the editor.

## ADDED Requirements

### Requirement: Region capture
The system SHALL freeze the current screen image and show a full-screen selection overlay. The overlay SHALL display the selection size in pixels, a magnifier loupe near the pointer, and SHALL snap to window edges when the pointer is within 8 px. Arrow keys SHALL move the selection by 1 px (Shift: 10 px). `Esc` SHALL cancel, `Enter` SHALL confirm.

#### Scenario: Select and confirm
- **WHEN** the user starts a region capture, drags a 400×300 rectangle and presses Enter
- **THEN** a 400×300 image of that area opens in the editor

#### Scenario: Cancel
- **WHEN** the user presses Esc in the overlay
- **THEN** no image is created and no window is opened

#### Scenario: Fixed size
- **WHEN** the user types a width and height into the overlay size field
- **THEN** the selection is set to exactly that size

### Requirement: Window capture
The system SHALL capture a single window including its shadow-free frame. On Wayland the window is chosen through the portal's interactive picker; on X11 by clicking the window in the overlay.

#### Scenario: Window on Wayland
- **WHEN** the user chooses window mode on GNOME Wayland
- **THEN** the portal picker is shown and the chosen window opens in the editor

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
The system SHALL register configurable global shortcuts for region (default `Print`), window (default `Alt+Print`) and screen (default `Shift+Print`) capture, offered as a reversible takeover of GNOME's own screenshot keys at first launch per decision D9 (`docs/design/decisions.md`). If the GlobalShortcuts portal is unavailable, Annota SHALL offer to create GNOME custom keybindings that run `annota --capture <mode>`.

#### Scenario: Hotkey on GNOME 46
- **WHEN** the GlobalShortcuts portal is unavailable and the user enables the hotkey in preferences
- **THEN** a GNOME custom keybinding for `Print` is created
- **AND** pressing it starts a region capture

### Requirement: Cursor inclusion
The user SHALL be able to include or exclude the mouse pointer; default is exclude.

#### Scenario: Include cursor
- **WHEN** include-cursor is enabled and the user captures the screen
- **THEN** the pointer is visible in the image

### Requirement: Image from clipboard or file
The system SHALL create a new document from an image in the clipboard (`Ctrl+Shift+V` in the main window) or from PNG/JPG/WebP files opened or dropped onto Annota.

#### Scenario: Paste
- **WHEN** the clipboard contains an image and the user presses Ctrl+Shift+V
- **THEN** a new editor tab opens with that image

### Requirement: Portal permission handling
If the screenshot portal denies access, the system SHALL show an explanatory dialog with a button that opens the relevant GNOME privacy settings.

#### Scenario: Permission denied
- **WHEN** the portal returns a permission error
- **THEN** the dialog explains how to grant access
