# Spec Delta

## MODIFIED Requirements

### Requirement: Autosave and recovery
Documents with unsaved changes SHALL be autosaved every 30 s as `.annota` files in `~/.cache/annota/autosave/` (only the user can read it), each recording the file it belongs to; nothing is written while a document does not change. An autosave SHALL be removed when its document is saved or back at its saved state, or discarded — in "Restore Unsaved Captures?" or in "Save Changes?" when it is closed. Autosaves left by a previous run SHALL be offered for recovery with name, autosave time, object count and thumbnail.

#### Scenario: Crash recovery
- **WHEN** Annota is killed with an unsaved document and restarted
- **THEN** the user is offered to restore the document, and Restore opens it in the editor with its changes

#### Scenario: Recoverable entry
- **WHEN** an unsaved document was autosaved and its autosave was never removed
- **THEN** the autosave directory lists it with name, autosave time, object count and thumbnail, and restoring it yields the same document and the file it belongs to

#### Scenario: Saved document leaves no autosave
- **WHEN** a document that has an autosave is saved
- **THEN** its autosave is removed at the next autosave tick

#### Scenario: Discarded when closing
- **WHEN** a document that has an autosave is closed with Discard in "Save Changes?"
- **THEN** its autosave is removed and not offered at the next start

### Requirement: Closing keeps autosaves
Closing the editor window (`Ctrl+W`, its close button) or quitting Annota SHALL remove a document's autosave only when the user saves or discards that document in "Save Changes?". When Annota ends without asking (it crashes, is killed, the session ends), its autosaves SHALL be kept and offered at the next start; if it still shuts down normally, it SHALL first write a last autosave of every document with unsaved changes. A new capture or pasted image counts as unsaved from the start, an image opened from a file only once it is changed.

#### Scenario: Ended without asking keeps the autosave
- **WHEN** Annota shuts down without having asked "Save Changes?" while a document has unsaved changes, some of them made after the last 30 s autosave
- **THEN** the document's autosave is kept, contains those changes, and is offered in "Restore Unsaved Captures?" at the next start

#### Scenario: Closed after saving or discarding
- **WHEN** the editor window closes after every unsaved document was saved or discarded in "Save Changes?"
- **THEN** none of these documents has an autosave left, and nothing is offered at the next start

### Requirement: Recovery dialog
When Annota starts and autosaves of a previous run are left — after a crash, or whenever Annota ended without asking "Save Changes?" — it SHALL show "Restore Unsaved Captures?" with one checked row per autosave. Its text SHALL say that these documents have changes not saved in a file and that Annota kept a copy of each — not that Annota crashed. Restore SHALL open the checked ones in the editor, Discard SHALL delete them; unchecked ones, or all if the dialog is closed otherwise, SHALL be kept and offered again.

#### Scenario: Restore
- **WHEN** the dialog lists an autosave and the user presses Restore
- **THEN** the document opens in the editor with its changes and stays recoverable until it is saved or discarded

#### Scenario: Discard
- **WHEN** the dialog lists an autosave and the user presses Discard
- **THEN** the autosave is deleted and not offered again

## ADDED Requirements

### Requirement: Save changes before closing
Closing the editor window (`Ctrl+W`, its close button) or quitting Annota (`Ctrl+Q`) SHALL ask "Save Changes?" (mockup 15) for each document with unsaved changes, one after the other in the tray's Open order, each document shown behind its alert: Save does what `Ctrl+S` does and closes it, Discard closes it without saving, Cancel stops the whole close or quit, and so does a save that fails. Documents without unsaved changes SHALL close silently.

#### Scenario: Document saved before
- **WHEN** the editor window is closed while "Checkout flow", which has its own `.annota` file, has unsaved changes
- **THEN** "Save Changes?" says "“Checkout flow” has unsaved changes. If you close without saving, they will be lost." with Cancel, Discard and Save, Save being the default

#### Scenario: Never saved
- **WHEN** the editor window is closed while a new capture that was never saved is open
- **THEN** the alert says "“<title>” has never been saved. If you close without saving, it will be lost."

#### Scenario: Several unsaved documents
- **WHEN** Annota quits while two documents have unsaved changes
- **THEN** one alert per document asks, in the tray's Open order, with that document shown behind it
- **AND** Cancel in the second alert keeps the second document open, while the first stays saved or discarded as answered

#### Scenario: Save
- **WHEN** the user chooses Save
- **THEN** the document is saved exactly as with Ctrl+S, its autosave is removed and it closes; after the last answer the window closes

#### Scenario: Save fails
- **WHEN** the user chooses Save and the document cannot be written
- **THEN** the window stays open, the document unchanged and unsaved with its autosave, the save's error toast is shown, and no further alert asks

#### Scenario: Discard
- **WHEN** the user chooses Discard
- **THEN** the document closes without being saved and its autosave is removed

#### Scenario: Cancel
- **WHEN** the user chooses Cancel or presses Esc
- **THEN** the window and every document stay open and nothing changes
