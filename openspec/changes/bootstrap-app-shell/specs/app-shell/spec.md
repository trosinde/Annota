# Spec Delta

## Purpose
Provides the application lifecycle, windows, settings and command-line entry points that every other capability builds on.

## ADDED Requirements

### Requirement: Single application instance
The application SHALL run as a single instance per user session; a second launch SHALL forward its command-line request to the running instance.

#### Scenario: Second launch forwards request
- **WHEN** Annota is running and the user runs `annota --capture region`
- **THEN** the running instance starts a region capture
- **AND** no second process remains running

### Requirement: Command-line interface
The `annota` binary SHALL accept `--capture region|window|screen`, `--delay <seconds>` (0–60), `--open <file>` and `--version`.

#### Scenario: Invalid mode
- **WHEN** the user runs `annota --capture banana`
- **THEN** the process exits with status 2 and prints the valid modes

#### Scenario: Open file
- **WHEN** the user runs `annota --open shot.annota`
- **THEN** the file opens in an editor window

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
