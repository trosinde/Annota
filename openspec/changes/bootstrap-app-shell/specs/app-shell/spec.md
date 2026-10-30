# Spec Delta

## Purpose
Provides the application lifecycle, windows, settings and command-line entry points that every other capability builds on.

## ADDED Requirements

### Requirement: Single application instance
The application SHALL run as a single instance per user session; a second launch SHALL forward its command-line request to the running instance, which SHALL act on it (this change raises the window and shows a toast for requests no other capability handles yet; `--capture` and `--open` get their real behaviour from the requirements below once `add-screen-capture` and `add-annotation-editor` exist).

#### Scenario: Second launch forwards request
- **WHEN** Annota is running and the user runs `annota` with any command-line arguments
- **THEN** the running instance raises its window and handles the request
- **AND** no second process remains running

### Requirement: Command-line interface
The `annota` binary SHALL accept `--capture region|window|screen`, `--delay <seconds>` (0–60), `--open <file>` and `--version`. Starting a capture and opening a file are delivered by `add-screen-capture` and `add-annotation-editor`; until then, the running instance acknowledges the request without performing it.

#### Scenario: Invalid mode
- **WHEN** the user runs `annota --capture banana`
- **THEN** the process exits with status 2 and prints the valid modes

#### Scenario: Open file acknowledged before the editor exists
- **WHEN** the user runs `annota --open shot.annota`
- **THEN** the running instance's window is raised and a toast says opening files is not available yet
- **AND** once `add-annotation-editor` ships, this scenario is superseded by that change's own "opens in an editor window" scenario via a `MODIFIED Requirements` delta here

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
