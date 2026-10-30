# Spec Delta

## Purpose
Gets finished images out of Annota into files, the clipboard and other applications.

## ADDED Requirements

### Requirement: Export formats
The system SHALL export the flattened document to PNG, JPG (quality 1–100), WebP (lossless/lossy) and PDF, and save editable `.annota`. Export output SHALL be pixel-identical to the canvas at 100 % zoom.

#### Scenario: WYSIWYG
- **WHEN** a document is exported to PNG
- **THEN** the PNG matches the canvas rendering at 100 % pixel by pixel

### Requirement: Quick save
`Ctrl+S` on a new capture SHALL save to the default folder using a filename pattern (default `Annota %Y-%m-%d %H-%M-%S`) without a dialog; `Ctrl+Shift+S` opens Save As.

#### Scenario: Pattern
- **WHEN** the user presses Ctrl+S on a new capture
- **THEN** a file named after the pattern is created in the default folder

### Requirement: Clipboard
`Ctrl+C` with no object selected SHALL copy the flattened image to the clipboard as PNG.

#### Scenario: Paste elsewhere
- **WHEN** the user presses Ctrl+C and pastes into another app
- **THEN** the annotated image is pasted

### Requirement: Drag and drop
Dragging the drag handle in the header bar SHALL provide the flattened image as a file to other applications.

#### Scenario: Drop into file manager
- **WHEN** the user drags the handle into a Files window
- **THEN** a PNG file is created there

### Requirement: Share
A Share menu SHALL offer `Email…` (portal Email with attachment) and `Open with…` (portal OpenURI).

#### Scenario: Email
- **WHEN** the user chooses Share → Email
- **THEN** the default mail client opens with the PNG attached

### Requirement: Batch export
Users SHALL select multiple documents in the tray and export them with the same format, size and filename pattern.

#### Scenario: Batch
- **WHEN** three documents are selected and exported as JPG
- **THEN** three JPG files are written
