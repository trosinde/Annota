# Spec Delta

## ADDED Requirements

### Requirement: Capture presets
Users SHALL define presets combining mode, fixed size, delay, cursor inclusion, quick effect and output action (editor, clipboard, file); each preset MAY have its own global shortcut.

#### Scenario: Preset hotkey
- **WHEN** a preset `800x600 to clipboard` has Ctrl+Print
- **THEN** pressing Ctrl+Print captures 800×600 and copies it without opening the editor

### Requirement: Multi-area capture
In region mode, holding `Ctrl` while selecting SHALL add further areas; the result is one image with the areas arranged in their screen positions on a transparent canvas.

#### Scenario: Two areas
- **WHEN** the user selects two separate areas with Ctrl
- **THEN** one image contains both areas

### Requirement: Time-lapse capture
Users SHALL capture the same region every N seconds (1–3600) until stopped, saving into a folder.

#### Scenario: Interval
- **WHEN** time-lapse with 10 s is started for 1 minute
- **THEN** 6 images are saved
