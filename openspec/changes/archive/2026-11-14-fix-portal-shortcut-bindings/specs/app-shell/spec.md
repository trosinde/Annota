## MODIFIED Requirements

### Requirement: Welcome page
Without a document, the main window SHALL show the welcome page (bootstrap-app-shell mockup 01). It holds "Capture Anything", a line saying that a capture opens in the editor, mode cards Region, Window and Screen, and a hint to open or drop images and `.annota` files. The first mode card has the keyboard focus. Each mode card shows the shortcut that actually starts that capture as keycaps, as defined in `capture` "Global capture hotkey": on the GlobalShortcuts portal path, the trigger the portal reported; on the GNOME-custom-keybinding fallback, the configured accelerator. A mode whose shortcut is not set shows no keycaps. A mode card SHALL start that capture, delivered by the after-capture destination (`capture`).

#### Scenario: Mode card starts a capture
- **WHEN** the user activates the Region card on the welcome page
- **THEN** a region capture starts, as with the Region shortcut

#### Scenario: Shortcut keycaps follow the settings
- **WHEN** the GlobalShortcuts portal is unavailable and the window-capture shortcut is changed in Preferences › Capture
- **THEN** the Window card shows the new shortcut

#### Scenario: Keycaps show the portal's triggers
- **WHEN** the GlobalShortcuts portal is available, it reports `region` bound to `Shift+Ctrl+P`, and Annota's region-capture setting is `Ctrl+P`
- **THEN** the Region card shows the keycaps `Shift` `Ctrl` `P`
