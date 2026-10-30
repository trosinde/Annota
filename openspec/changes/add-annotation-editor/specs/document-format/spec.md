# Spec Delta

## Purpose
Defines how Annota documents are stored so that annotations remain editable across sessions and versions.

## ADDED Requirements

### Requirement: Editable document file
Documents SHALL be saved as `.annota` ZIP files containing `original.png`, `document.json` (versioned scene graph) and `thumbnail.png`; loading SHALL restore every object with all properties.

#### Scenario: Round trip
- **WHEN** a document with one of every object type is saved and reopened
- **THEN** all objects and properties are identical

### Requirement: Forward compatibility
`document.json` SHALL contain a schema version; files written by a newer minor version SHALL open with unknown properties preserved, and newer major versions SHALL show a clear error.

#### Scenario: Newer major
- **WHEN** the user opens a file with schema major version higher than supported
- **THEN** an error names the required Annota version

### Requirement: Autosave and recovery
Unsaved documents SHALL be autosaved every 30 s to the user cache directory and offered for recovery after a crash.

#### Scenario: Crash recovery
- **WHEN** Annota is killed with an unsaved document and restarted
- **THEN** the user is offered to restore the document
