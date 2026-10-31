# Spec Delta

## Purpose
Gets finished images out of Annota into files, the clipboard and other applications.

## ADDED Requirements

### Requirement: Export formats
The system SHALL export the flattened document to PNG, JPG (quality 1–100), WebP (lossless/lossy) and PDF, and save editable `.annota`. Export output SHALL be pixel-identical to the canvas at 100 % zoom.

#### Scenario: WYSIWYG
- **WHEN** a document is exported to PNG
- **THEN** the PNG matches the canvas rendering at 100 % pixel by pixel

### Requirement: Export dialog
`Ctrl+E` SHALL open "Export Image" (mockups 02–05) with file name, folder (`export-folder` preference), format (`default-export-format` preference), the format's settings and a size. Before anything is written it SHALL show a banner when the folder is not writable or a file of that name exists, and keep Export disabled until that is resolved.

#### Scenario: Folder not writable
- **WHEN** the export dialog's folder is one the user may not write to
- **THEN** the banner "No permission to write to this folder" with Choose Folder… is shown and Export is disabled

#### Scenario: File exists
- **WHEN** a file with the dialog's file name and the format's extension already exists in its folder
- **THEN** the banner "A file with this name already exists" with Replace is shown, Export is disabled, and Replace writes over the file

### Requirement: Quick save
`Ctrl+S` SHALL save the document as an editable `.annota` file without a dialog: over its own `.annota` file if it has one, otherwise into the default folder (`save-folder` preference) named after the filename pattern (`filename-pattern` preference, default `Annota %Y-%m-%d %H-%M-%S`), never replacing another file. `Ctrl+Shift+S` SHALL open Save As.

#### Scenario: Pattern
- **WHEN** the user presses Ctrl+S on a new capture
- **THEN** a file named after the pattern is created in the default folder

#### Scenario: Saved over its file
- **WHEN** the user edits a document that was saved before and presses Ctrl+S
- **THEN** its `.annota` file is replaced and no new file is created

### Requirement: Saved document
A saved document SHALL take its file's name as title, lose its autosave and announce the file in a toast with Show in Files (mockup 06). If saving fails, the document SHALL stay open and unsaved and a toast SHALL say why.

#### Scenario: Saved
- **WHEN** a new capture is saved with Ctrl+S
- **THEN** the window title is the file's name, the subtitle "<folder> · Saved", its autosave is removed, and the toast "Saved as “<file name>”" offers Show in Files

#### Scenario: Save fails
- **WHEN** the user presses Ctrl+S on a new capture and the default folder cannot be written
- **THEN** a toast says "No permission to write to this folder", the document stays "Not saved" and keeps its autosave

### Requirement: Clipboard
`Ctrl+C` with no object selected, and the header's Copy, SHALL copy the flattened image to the clipboard as PNG and confirm with the toast "Image copied to clipboard" (mockup 07, 3 s). Copying SHALL not count as saving the document.

#### Scenario: Paste elsewhere
- **WHEN** the user presses Ctrl+C and pastes into another app
- **THEN** the annotated image is pasted

### Requirement: Drag and drop
Dragging the window title area (the header bar's title and subtitle, decision D1) SHALL provide the flattened image to other applications as a PNG file named after the document (`text/uri-list`) and as `image/png` data. Hovering the title area SHALL show the tooltip "Drag image to another app" (mockup 08); the drag image is a thumbnail with a "PNG" badge (mockup 09).

#### Scenario: Drop into file manager
- **WHEN** the user drags the title area of "Checkout flow" into a Files window
- **THEN** a PNG file "Checkout flow.png" with the flattened image is created there

### Requirement: Share
The dropdown of the Export split button (Share, decision D1) SHALL offer `Email…` (portal Email, the flattened PNG attached), `Open With…` (portal OpenURI with the system's app chooser) and `Copy to Folder…` (a file chooser, then the flattened PNG written there). The PNG is named after the document. Cancelling a portal's own dialog SHALL not be reported as an error.

#### Scenario: Email
- **WHEN** the user chooses Share → Email
- **THEN** the default mail client opens with the PNG attached
- **AND** while the mail app is being asked, the toast "Opening your mail app with “<name>.png” attached…" is shown (mockup 14)

#### Scenario: Open with
- **WHEN** the user chooses Share → Open With… and picks an app in the system's app chooser
- **THEN** that app opens the PNG

#### Scenario: Copy to folder
- **WHEN** the user chooses Share → Copy to Folder… and confirms a folder and name in the file chooser
- **THEN** the flattened image is written there as PNG, a toast "Copied as “<name>.png”" offers Show in Files, and the document stays unsaved

### Requirement: Batch export
Users SHALL select multiple documents in the tray and export them with the same format, size and filename pattern.

#### Scenario: Batch
- **WHEN** three documents are selected and exported as JPG
- **THEN** three JPG files are written
