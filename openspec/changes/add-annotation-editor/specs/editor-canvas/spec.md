# Spec Delta

## Purpose
Hosts the capture and its annotation objects and provides all object manipulation, view and history functions.

## ADDED Requirements

### Requirement: Editor layout
The editor window SHALL show a header bar with tool buttons, a central canvas, a collapsible properties sidebar (right) and a tray of open/recent captures (bottom). Width ≥ 900 px shows all; narrower windows SHALL collapse the sidebar into a bottom sheet.

#### Scenario: Narrow window
- **WHEN** the editor window is resized to 700 px width
- **THEN** the properties panel moves into a bottom sheet

### Requirement: Object selection and manipulation
Users SHALL select objects by click, Shift+click (add) or rubber-band; move by drag or arrow keys (Shift = 10 px); resize via 8 handles (Shift keeps ratio); rotate via a rotation handle (Shift = 15° steps); duplicate with `Ctrl+D`; delete with `Del`.

#### Scenario: Keyboard nudge
- **WHEN** an object is selected and the user presses Shift+→
- **THEN** the object moves 10 px right

#### Scenario: Rotate snapping
- **WHEN** the user rotates an object while holding Shift
- **THEN** the angle snaps to multiples of 15°

### Requirement: Select all of a type
The properties sidebar SHALL offer `Select all <type>` to select every object of the current type and edit their shared properties at once.

#### Scenario: Recolour all arrows
- **WHEN** the document has 5 arrows and the user uses Select all arrows and picks red
- **THEN** all 5 arrows turn red

### Requirement: Z-order and grouping
Users SHALL change stacking with `Ctrl+]`/`Ctrl+[` (one step) and `Ctrl+Shift+]`/`Ctrl+Shift+[` (front/back), group with `Ctrl+G`, ungroup with `Ctrl+Shift+G`.

#### Scenario: Group moves together
- **WHEN** two objects are grouped and the user drags one
- **THEN** both move together

### Requirement: Snapping and guides
While moving, objects SHALL snap to canvas edges, canvas centre and other objects' edges/centres within 6 screen px, showing guide lines. Holding `Alt` SHALL suspend snapping; `Ctrl+;` toggles snapping.

#### Scenario: Centre snap
- **WHEN** the user drags an object near the canvas centre
- **THEN** a guide line appears and the object snaps to centre

### Requirement: View control
Users SHALL zoom with `Ctrl+scroll`, `Ctrl++`/`Ctrl+-`, `Ctrl+0` (fit) and `Ctrl+1` (100 %), and pan with space+drag or middle mouse; zoom range 10–1600 %.

#### Scenario: Fit
- **WHEN** the user presses Ctrl+0
- **THEN** the whole image is visible and centred

### Requirement: Undo and redo
Every document change SHALL be undoable with `Ctrl+Z` and redoable with `Ctrl+Shift+Z`, with at least 200 steps per document.

#### Scenario: Undo move
- **WHEN** the user moves an object and presses Ctrl+Z
- **THEN** the object returns to its previous position

### Requirement: Image operations
Users SHALL crop (`C`), resize the image with optional aspect lock, rotate 90°/180° and flip horizontally/vertically; the image SHALL also accept rounded corners via a corner-radius handle.

#### Scenario: Crop
- **WHEN** the user selects the crop tool, drags the crop handles and presses Enter
- **THEN** the image and canvas are cropped and annotations keep their image positions

### Requirement: Canvas extension
Users SHALL extend the canvas beyond the image by dragging canvas edges; the canvas colour SHALL be configurable including transparent.

#### Scenario: Extend for callout
- **WHEN** the user drags the right canvas edge 200 px outward
- **THEN** the canvas grows by 200 px filled with the canvas colour

### Requirement: Flatten
Users SHALL flatten the selected objects (`Ctrl+T`) or all objects (`Ctrl+Shift+T`) into the image; the action SHALL be undoable.

#### Scenario: Flatten all
- **WHEN** the user presses Ctrl+Shift+T
- **THEN** no editable objects remain and the image shows them burnt in

### Requirement: Recent captures tray
The tray SHALL show thumbnails of open documents and the last 50 captures; clicking opens, dragging a thumbnail onto the canvas inserts it as an image object.

#### Scenario: Picture in picture
- **WHEN** the user drags a tray thumbnail onto the canvas
- **THEN** the image is inserted as a movable object
