# Spec Delta

## MODIFIED Requirements

### Requirement: Single application instance
The application SHALL run as a single instance per user session; a second launch SHALL forward its command-line request to the running instance, which SHALL act on it. `--capture <mode>` starts that capture (capability `capture`) without raising the main window first, so Annota's own window is not part of the capture. Other requests raise the window; `--open` keeps acknowledging with a toast until `add-annotation-editor` gives it its real behaviour.

#### Scenario: Second launch forwards request
- **WHEN** Annota is running and the user runs `annota` with valid arguments other than `--version` and `--capture`
- **THEN** the running instance raises its window and handles the request
- **AND** no second process remains running

#### Scenario: Second launch starts a capture
- **WHEN** Annota is running and the user runs `annota --capture region` (or `window`, `screen`)
- **THEN** the running instance starts a region (or window, screen) capture without raising its window first
- **AND** no second process remains running
