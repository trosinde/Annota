# Spec Delta

## MODIFIED Requirements

### Requirement: Image from clipboard or file
The system SHALL create a new capture from an image in the clipboard (`Ctrl+Shift+V`) or from PNG/JPG/WebP/`.annota` files opened (`Ctrl+O`) or dropped onto Annota, and open it in the editor as a new document (add-screen-capture mockup 13). A toast SHALL offer Undo, which closes the new document again. A file that is already open SHALL become the current document instead of being opened twice.

#### Scenario: Paste
- **WHEN** the clipboard contains an image and the user presses Ctrl+Shift+V
- **THEN** the editor shows a new document "Pasted Image" with that image, `W × H · Not saved`, and a toast "New capture from clipboard" with Undo

#### Scenario: Drop an image file
- **WHEN** the user drops a PNG file onto the Annota window
- **THEN** the editor shows a new document with that image, named after the file, and a toast "New capture from file" with Undo

#### Scenario: Unsupported file
- **WHEN** the user drops a file that is not PNG, JPG, WebP or `.annota`
- **THEN** no capture is created and a toast says which formats can be opened

#### Scenario: File already open
- **WHEN** the user opens or drops a file that is already open in the editor
- **THEN** that document becomes the current one instead of a duplicate being created

### Requirement: After-capture destination
The system SHALL deliver every capture per the `after-capture` preference: "Copy to Clipboard" puts it on the clipboard and opens no window; "Open in Editor" opens it in the editor as a new document (add-screen-capture mockup 12); "Both" does both. A new capture is autosaved like any unsaved document (`document-format`) until saved or exported (add-export-share).

#### Scenario: Copy to clipboard
- **WHEN** the after-capture preference is "Copy to Clipboard" and the user captures a region with the global shortcut while no Annota window is open
- **THEN** the image is on the clipboard
- **AND** no window opens

#### Scenario: Editor destination
- **WHEN** the after-capture preference is "Open in Editor" and the user captures a 400×300 region
- **THEN** the editor window shows a new 400 × 300 document, subtitle `400 × 300 · Not saved`, as the current one
- **AND** a toast "Region captured · 400 × 300" offers Copy

#### Scenario: Both
- **WHEN** the after-capture preference is "Both" and the user captures a region
- **THEN** the capture opens in the editor and the image is on the clipboard

#### Scenario: Editor unavailable
- **WHEN** the after-capture preference includes "Open in Editor" but the editor window cannot be shown
- **THEN** the capture is saved as PNG into the save folder instead, never overwriting a file, with a toast naming it
