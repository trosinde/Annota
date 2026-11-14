## MODIFIED Requirements

### Requirement: Global capture hotkey
The system SHALL register global shortcuts for region (default `Print`), window (default `Alt+Print`) and screen (default `Shift+Print`) capture.

With the GlobalShortcuts portal (GNOME ≥ 48), Annota binds them through the portal while it runs, using the IDs `region`, `window` and `screen` and its configured accelerators as preferred triggers. The desktop decides the triggers and keeps them: the portal's answer to the bind is the only source of the triggers Annota shows. On this path:
- Preferences › Capture, the capture launcher and the welcome page SHALL show, for each mode, the trigger the portal reported for that ID, or "Not set" if the portal reported none. They SHALL NOT show Annota's configured accelerator instead.
- Preferences › Capture SHALL NOT open the Set Shortcut dialog. It SHALL say that GNOME manages these shortcuts and offer "Open GNOME Settings", which opens GNOME Settings at Annota's page under Apps, where its global shortcuts are listed. Annota opens no network connection for this.
- Annota SHALL read the portal's triggers again whenever it binds, and whenever Preferences › Capture or the capture launcher opens. A trigger changed in GNOME Settings then shows without restarting Annota, as soon as the portal reports it.
- Until the portal has answered, the shortcut rows and keycaps show no trigger, not Annota's configured accelerator.
- A reported trigger that GNOME's own keybinding settings also assign to a GNOME action is taken by GNOME: GNOME Shell refuses Annota's grab, the key runs GNOME's action, and the portal does not tell Annota. Annota SHALL still show that trigger, marked as used by GNOME and naming the GNOME action ("Used by GNOME: <action>"), and SHALL point to GNOME Settings to change it. It SHALL find these collisions only in GNOME's public keybinding settings, the same ones the Set Shortcut dialog checks.

If the portal is unavailable, Annota SHALL offer in Preferences › Capture to create GNOME custom keybindings that run `annota --capture <mode>`. On that path the configured accelerators are the bound keys and are shown and edited as before.

#### Scenario: Hotkey on GNOME 46
- **WHEN** the GlobalShortcuts portal is unavailable and the user enables the hotkey in preferences
- **THEN** a GNOME custom keybinding for `Print` is created
- **AND** pressing it starts a region capture

#### Scenario: Portal keeps an earlier trigger
- **WHEN** the GlobalShortcuts portal is available, GNOME has bound `screen` to `Shift+Print`, and Annota's screen-capture setting is `Ctrl+Q`
- **THEN** Preferences › Capture, the launcher's Screen row and the welcome page's Screen card show `Shift+Print`
- **AND** none of them shows `Ctrl+Q`

#### Scenario: Shortcut the portal has not bound
- **WHEN** the GlobalShortcuts portal is available and its answer to the bind contains no trigger for `window`
- **THEN** the Window capture row in Preferences › Capture reads "Not set", and the launcher and the welcome page show no keycap for Window

#### Scenario: Shortcuts are managed by GNOME
- **WHEN** the GlobalShortcuts portal is available and the user activates the Region capture row in Preferences › Capture
- **THEN** no Set Shortcut dialog opens
- **AND** the group explains that GNOME manages these shortcuts and offers "Open GNOME Settings"

#### Scenario: Open GNOME Settings
- **WHEN** the user clicks "Open GNOME Settings" in Preferences › Capture
- **THEN** GNOME Settings opens at Apps › Annota, which has the "Global Shortcuts" row listing Annota's shortcuts

#### Scenario: Trigger taken by GNOME
- **WHEN** the GlobalShortcuts portal is available, it reports `screen` bound to `Shift+Print`, and GNOME's keybinding settings assign `Shift+Print` to "Take a screenshot"
- **THEN** the Screen capture row in Preferences › Capture shows `Shift+Print` with "Used by GNOME: Take a screenshot"
- **AND** the launcher's Screen row and the welcome page's Screen card mark their `Shift+Print` keycaps as used by GNOME
- **AND** pressing `Shift+Print` runs GNOME's screenshot, not an Annota capture

#### Scenario: Trigger changed in GNOME Settings
- **WHEN** Annota is running on the portal path and the user changes Annota's screen-capture shortcut in GNOME Settings to `Super+Shift+S`, then opens Preferences › Capture
- **THEN** the Screen capture row shows `Super+Shift+S`, provided the portal reports that trigger
- **AND** pressing `Super+Shift+S` starts a screen capture

### Requirement: Reversible takeover of GNOME's screenshot keys
Per decision D9, the default shortcuts are GNOME's own screenshot keys. On the GNOME-custom-keybinding fallback only (see "Global capture hotkey"), when Annota creates those keybindings the system SHALL move GNOME's bindings for the same keys aside, recorded and never deleted, and SHALL give them back when the Annota shortcut is disabled or changed.

The Set Shortcut dialog SHALL refuse a key combination that is already in use:
- by another GNOME action, other than GNOME's own screenshot keys, which Annota takes over;
- by another of Annota's capture shortcuts;
- by one of Annota's own application or window shortcuts (e.g. `Ctrl+Q` Quit, `Ctrl+O` Open, `Ctrl+L` Library, `Ctrl+,` Preferences, `Shift+Ctrl+V` Paste). A global binding on one of these would take the key away from every application, Annota included.

#### Scenario: Takeover is reversible
- **WHEN** Annota's GNOME shortcuts were created and the user disables the region shortcut with Backspace in the Set Shortcut dialog
- **THEN** GNOME's own `Print` binding is restored

#### Scenario: No takeover on the GlobalShortcuts portal path
- **WHEN** the GlobalShortcuts portal is available (GNOME ≥ 48, e.g. Debian 13, Ubuntu 26.04) and Annota binds its shortcuts
- **THEN** GNOME Shell's own bindings for `Print`, `Alt+Print` and `Shift+Print` are left untouched
- **AND** the triggers Annota shows are the ones the portal reported
- **AND** a reported trigger that is one of those GNOME keys is marked "Used by GNOME" (see "Global capture hotkey")

#### Scenario: Shortcut conflict
- **WHEN** the user presses a key combination in the Set Shortcut dialog that another GNOME action already uses (other than GNOME's own screenshot keys, which Annota takes over)
- **THEN** the dialog names that action and "Set" stays insensitive

#### Scenario: Conflict with an Annota application shortcut
- **WHEN** the user presses `Ctrl+Q` in the Set Shortcut dialog for Screen capture
- **THEN** the dialog reads "Used by Annota: Quit" and "Set" stays insensitive
