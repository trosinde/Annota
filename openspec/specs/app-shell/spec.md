# app-shell Specification

## Purpose
Provides the application lifecycle, windows, settings and command-line entry points that every other capability builds on.

## Requirements

### Requirement: Single application instance
The application SHALL run as a single instance per user session; a second launch SHALL forward its command-line request to the running instance, which SHALL act on it. `--capture <mode>` starts that capture (capability `capture`) without raising the main window first, so Annota's own window is not part of the capture. `--open <file>` opens the file in the editor window (see "Command-line options"). Other requests raise the main window.

#### Scenario: Second launch forwards request
- **WHEN** Annota is running and the user runs `annota` with valid arguments other than `--version` and `--capture`
- **THEN** the running instance raises its window and handles the request
- **AND** no second process remains running

#### Scenario: Second launch starts a capture
- **WHEN** Annota is running and the user runs `annota --capture region` (or `window`, `screen`)
- **THEN** the running instance starts a region (or window, screen) capture without raising its window first
- **AND** no second process remains running

### Requirement: Preferences
The application SHALL provide a libadwaita preferences window persisted via GSettings schema `io.github.trosinde.Annota`, including default save folder, default export format, and after-capture behaviour (open editor / copy to clipboard / both).

#### Scenario: Setting persists
- **WHEN** the user changes the default export format to JPG and restarts Annota
- **THEN** JPG is still selected

### Requirement: Keyboard shortcuts window
The application SHALL show all shortcuts in a GTK shortcuts window opened with `Ctrl+?`.

#### Scenario: Open shortcuts
- **WHEN** the user presses `Ctrl+?` in any Annota window
- **THEN** the shortcuts window opens

### Requirement: Light and dark style
The application SHALL follow the system light/dark preference and the system accent color.

#### Scenario: System switches to dark
- **WHEN** the user switches GNOME to dark style while Annota is open
- **THEN** all Annota windows switch to the dark style without restart

### Requirement: Local-only operation
Annota SHALL work completely offline and SHALL NOT require, offer or contact any user account, sign-in, cloud service, licence server, update server or telemetry endpoint. Network access SHALL only happen as a side effect of actions the user starts with another local application (for example handing an image to the system mail client).

#### Scenario: No network at all
- **WHEN** Annota runs on a machine without any network connection
- **THEN** every feature works and no dialog mentions sign-in or connectivity

#### Scenario: No outgoing connections
- **WHEN** Annota is used for capturing, editing, saving and exporting while outgoing traffic is monitored
- **THEN** Annota opens no network connection

### Requirement: Command-line options
The `annota` binary SHALL accept `--capture region|window|screen`, `--delay <seconds>` (0–60), `--open <file>` and `--version`. Starting a capture is delivered by `capture`. `--open <file>` SHALL open an `.annota` document or a PNG, JPG or WebP image (relative to the directory `annota` was started in) in the editor window and present it; a file that is open already becomes the current document.

#### Scenario: Invalid mode
- **WHEN** the user runs `annota --capture banana`
- **THEN** the process exits with status 2 and prints the valid modes

#### Scenario: Open file
- **WHEN** the user runs `annota --open shot.annota`
- **THEN** the editor window opens (or is raised) with `shot.annota` as the current document, titled with the document's name, its subtitle `W × H` without "Edited"

#### Scenario: Open image file
- **WHEN** the user runs `annota --open shot.png`
- **THEN** the editor window shows `shot.png` as a new document titled "shot", subtitle `W × H · Not saved`

#### Scenario: Open a file of a newer format
- **WHEN** the user runs `annota --open` with a file of a newer major format version
- **THEN** the editor shows the "Newer Annota Version Needed" alert and the file is unchanged

### Requirement: Welcome page
Without a document, the main window SHALL show the welcome page (bootstrap-app-shell mockup 01): "Capture Anything", a line saying that a capture opens in the editor, mode cards Region, Window and Screen with their current shortcuts as keycaps (the first one focused), and a hint to open or drop images and `.annota` files. A mode card SHALL start that capture, delivered by the after-capture destination (`capture`).

#### Scenario: Mode card starts a capture
- **WHEN** the user activates the Region card on the welcome page
- **THEN** a region capture starts, as with the Region shortcut

#### Scenario: Shortcut keycaps follow the settings
- **WHEN** the window-capture shortcut is changed in Preferences › Capture
- **THEN** the Window card shows the new shortcut
