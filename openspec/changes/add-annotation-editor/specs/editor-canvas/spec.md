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
Users SHALL zoom with `Ctrl+scroll` (around the pointer), `Ctrl++`/`Ctrl+-` (through the zoom presets 25, 50, 100, 200, 400, 800, 1600 %), `Ctrl+0` (fit) and `Ctrl+1` (100 %), and pan with space+drag, middle mouse or the scroll bars; zoom range 10–1600 %. Fit SHALL show the whole canvas centred with a 44 px margin, never above 100 %, and SHALL keep fitting while the window is resized.

#### Scenario: Fit
- **WHEN** the user presses Ctrl+0
- **THEN** the whole canvas is visible and centred, at most at 100 %

#### Scenario: Zoom at the pointer
- **WHEN** the user zooms with Ctrl+scroll
- **THEN** the point of the document under the pointer stays under the pointer

#### Scenario: Fit follows the window
- **WHEN** the canvas is fitted and the window is made smaller (for example from 1280 × 800 to 900 × 600)
- **THEN** the whole canvas stays visible and centred at the new fit zoom

### Requirement: Shared renderer
The canvas SHALL draw the document with the same renderer as export, so that at 100 % zoom the canvas shows exactly the exported pixels. A transparent canvas SHALL appear as a checkerboard on screen and stay transparent in exports. Only objects inside the visible area SHALL be drawn, using a spatial index for documents with more than 100 objects.

#### Scenario: WYSIWYG
- **WHEN** a document is shown on the canvas at 100 % zoom
- **THEN** the canvas pixels of the document equal the pixels of its export

#### Scenario: Transparent canvas
- **WHEN** the canvas background is transparent
- **THEN** the canvas shows a checkerboard there and the export has transparent pixels there

#### Scenario: Only the visible area is drawn
- **WHEN** a document with 300 objects is zoomed to 1600 %
- **THEN** a redraw covers only the visible area of the view, not the whole zoomed document, and draws only the objects in it

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
