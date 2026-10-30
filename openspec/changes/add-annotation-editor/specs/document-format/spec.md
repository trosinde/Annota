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

### Requirement: Documents stay saveable
Every change to a document SHALL keep it saveable and reopenable. A change that would give an object a position, size or rotation that is not a finite number, make it use an embedded image the document does not have, or give it a value that cannot be written and read back SHALL be refused with an error, and the document SHALL stay unchanged.

#### Scenario: Invalid change refused
- **WHEN** an edit would set an object coordinate to a non-finite value (for example a size computed by dividing by zero) or point an object at an embedded image that does not exist
- **THEN** the edit fails with an error, the document is unchanged, and it still saves, autosaves and reopens

### Requirement: Unique object ids
Every object SHALL have an id that is unique in the whole document, members of groups included, and ids SHALL never be handed out again within a session. A change that would give an object an id another object already has SHALL be refused with an error, and the document SHALL stay unchanged; a file with duplicate ids SHALL NOT open.

#### Scenario: Duplicate id refused
- **WHEN** an edit would insert a group whose member has the id of an object already in the document (top-level or inside another group)
- **THEN** the edit fails with an error and the document is unchanged

#### Scenario: Group ids survive saving
- **WHEN** objects are grouped, the document is saved and reopened, and the group is ungrouped
- **THEN** the objects come back with their ids, and new objects get ids no object has had

### Requirement: Format versions
`document.json` SHALL contain its format version as `"format": "MAJOR.MINOR"` and the oldest Annota version that reads it as `"requires"`. Files of a newer minor version SHALL open without any message. Files of a newer major version SHALL NOT be opened: the error SHALL name the file's format version and the Annota version it requires, and the file SHALL NOT be changed.

#### Scenario: Newer major
- **WHEN** the user opens a file whose format major version is higher than supported
- **THEN** opening fails with an error that names the file's format version and the required Annota version (shown as the "Newer Annota Version Needed" alert of mockup 15), and the file is unchanged

### Requirement: Forward compatibility
When a file of a newer minor version is saved, everything this version does not know SHALL be kept and written back: unknown keys at any depth (top level, inside sections such as `canvas.background`, inside objects and their nested values such as `shadow`), objects of unknown types, and the embedded images such objects use. Known keys SHALL be written with their current values. When a value with a `"type"` (such as the background) changes type, the unknown keys of its old type SHALL be dropped.

#### Scenario: Newer minor
- **WHEN** the user opens a file of a newer minor format version that contains properties and object types this version does not know, and saves it
- **THEN** the file opens without a message and the saved file still contains those properties and objects

#### Scenario: Unknown nested keys
- **WHEN** a file of a newer minor version has unknown keys inside `canvas.background` and inside an object's `shadow`, and the user moves that object and saves
- **THEN** the saved file contains those keys unchanged, and the object's new position

### Requirement: Unknown values
A value this version cannot interpret SHALL NOT prevent a file of a newer minor version from opening. An unknown canvas background type SHALL be shown and exported as transparent and written back unchanged until the user picks another background. An object of a known type that cannot be read SHALL be kept like an object of an unknown type: not drawn, not selectable, written back unchanged. In files of this or an older minor version such values are damage, and the file SHALL NOT open.

#### Scenario: Unknown background type
- **WHEN** a file of a newer minor version has a canvas background whose `"type"` this version does not know
- **THEN** the file opens, the background is shown and exported as transparent, and saving writes the background back unchanged

#### Scenario: Unreadable object in a newer minor file
- **WHEN** a file of a newer minor version contains an object of a known type with a value this version cannot read
- **THEN** the file opens, that object is kept and written back unchanged together with its embedded image, and the other objects can be edited

### Requirement: Groups with unknown members
A group that contains an object of a type this version does not know SHALL be kept as a whole like an object of an unknown type: not drawn, not selectable, written back unchanged, so that it is never moved without its unknown parts.

#### Scenario: Group with an unknown member
- **WHEN** a file contains a group one of whose members has a type this version does not know
- **THEN** the file opens and the whole group is kept and written back unchanged, not drawn and not selectable

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
