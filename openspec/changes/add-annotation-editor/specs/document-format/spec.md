# Spec Delta

## Purpose
Defines how Annota documents are stored so that annotations remain editable across sessions and versions.

## ADDED Requirements

### Requirement: Editable document file
Documents SHALL be saved as `.annota` ZIP files containing `document.json` (versioned scene graph, schema `data/schema/document-1.schema.json`), `original.png`, `thumbnail.png` (the flattened canvas, at most 512 px on the longer side) and one `images/<n>.png` per embedded image that an image object uses. Saving SHALL replace an existing file only once the new file is complete. Loading SHALL restore every object with all properties.

#### Scenario: Round trip
- **WHEN** a document with one of every object type is saved and reopened
- **THEN** all objects and properties are identical

#### Scenario: Failed save
- **WHEN** saving over an existing file fails part-way (for example because the folder is not writable)
- **THEN** the existing file is unchanged

### Requirement: Forward compatibility
`document.json` SHALL contain its format version as `"format": "MAJOR.MINOR"` and the oldest Annota version that reads it as `"requires"`. Files of a newer minor version SHALL open without any message; keys and object types this version does not know SHALL be kept and written back when the document is saved. Files of a newer major version SHALL NOT be opened: the error SHALL name the file's format version and the Annota version it requires, and the file SHALL NOT be changed.

#### Scenario: Newer major
- **WHEN** the user opens a file whose format major version is higher than supported
- **THEN** opening fails with an error that names the file's format version and the required Annota version (shown as the "Newer Annota Version Needed" alert of mockup 15), and the file is unchanged

#### Scenario: Newer minor
- **WHEN** the user opens a file of a newer minor format version that contains properties and object types this version does not know, and saves it
- **THEN** the file opens without a message and the saved file still contains those properties and objects

### Requirement: Autosave and recovery
Documents with unsaved changes SHALL be autosaved every 30 s as `.annota` files in `~/.cache/annota/autosave/` (a directory only the user can read), each also recording the file the document belongs to. Nothing SHALL be written while a document does not change, and a document's autosave SHALL be removed when it is saved, closed, or back at its saved state. Autosaves left over from a previous run SHALL be offered for recovery, each with its name, autosave time, object count and thumbnail.

#### Scenario: Crash recovery
- **WHEN** Annota is killed with an unsaved document and restarted
- **THEN** the user is offered to restore the document

#### Scenario: Recoverable entry
- **WHEN** an unsaved document was autosaved and its autosave was never removed
- **THEN** the autosave directory lists it with name, autosave time, object count and thumbnail, and restoring it yields the same document and the file it belongs to

#### Scenario: Saved document leaves no autosave
- **WHEN** a document that has an autosave is saved
- **THEN** its autosave is removed at the next autosave tick
