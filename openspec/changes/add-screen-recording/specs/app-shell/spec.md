# Spec Delta

## MODIFIED Requirements

### Requirement: Welcome page
Without a document, the main window SHALL show the welcome page (bootstrap-app-shell mockup 01): "Capture Anything", a line saying that a capture opens in the editor, mode cards Region, Window and Screen (the first one focused) followed by a Record card (add-screen-recording mockup 12), and a hint to open or drop images and `.annota` files. Each card SHALL show as keycaps the shortcut that actually starts that capture or recording (`capture` "Global capture hotkey", `screen-recording` "Record shortcut"), none if unset, and SHALL start it.

#### Scenario: Mode card starts a capture
- **WHEN** the user activates the Region card on the welcome page
- **THEN** a region capture starts, as with the Region shortcut

#### Scenario: Shortcut keycaps follow the settings
- **WHEN** the GlobalShortcuts portal is unavailable and the window-capture shortcut is changed in Preferences › Capture
- **THEN** the Window card shows the new shortcut

#### Scenario: Keycaps show the portal's triggers
- **WHEN** the GlobalShortcuts portal is available, it reports `region` bound to `Shift+Ctrl+P`, and Annota's region-capture setting is `Ctrl+P`
- **THEN** the Region card shows the keycaps `Shift` `Ctrl` `P`

#### Scenario: Record card starts the recording setup
- **WHEN** the user activates the Record card on the welcome page
- **THEN** the region overlay opens in Video mode with the recording setup, as with the Record shortcut
