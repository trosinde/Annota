# Spec Delta

## MODIFIED Requirements

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

## REMOVED Requirements

### Requirement: Command-line interface
**Reason**: Its scenario "Open file acknowledged before the editor exists" described a stand-in that ends with this change; a MODIFIED block cannot drop a scenario.
**Migration**: Replaced by the ADDED requirement "Command-line options" below — the whole block carried over (the `--capture`/`--delay`/`--open`/`--version` contract and the "Invalid mode" scenario), with `--open` given its real behaviour. A new name because a requirement cannot be both removed and added in one delta.

## ADDED Requirements

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
