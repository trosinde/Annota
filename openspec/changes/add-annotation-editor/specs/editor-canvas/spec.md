# Spec Delta

## Purpose
Hosts the capture and its annotation objects and provides all object manipulation, view and history functions.

## ADDED Requirements

### Requirement: Editor layout
The editor window SHALL show a header bar with tool buttons, a central canvas, a collapsible properties sidebar (right) and a tray of open/recent captures (bottom). Width ≥ 900 px shows all; narrower windows SHALL collapse the sidebar into a bottom sheet.

#### Scenario: Narrow window
- **WHEN** the editor window is resized to 700 px width
- **THEN** the properties panel moves into a bottom sheet

### Requirement: Object selection
Users SHALL select objects by click, Shift+click (add), Ctrl+click (toggle) or rubber band, which selects the objects whose box lies entirely inside it (Shift adds, Ctrl toggles); `Ctrl+A` selects all and `Esc` deselects. Selecting is not a document change and not an undo step.

#### Scenario: Add to the selection
- **WHEN** one object is selected and the user Shift+clicks a second one and drags either
- **THEN** both are selected and both move

#### Scenario: Rubber band
- **WHEN** the user drags on empty canvas around two objects and only partly over a third
- **THEN** exactly the two enclosed objects are selected

### Requirement: Object manipulation
Users SHALL move the selection by drag or arrow keys (1 px, Shift = 10 px); resize via 8 handles (Shift keeps the ratio of shapes; images keep theirs unless Shift is held, mockup 12); rotate via a rotation handle with a live angle chip (Shift = 15° steps); duplicate with `Ctrl+D` (copies offset by 10 px become the selection); delete with `Del`. Several selected objects SHALL be resized and rotated as one box.

#### Scenario: Keyboard nudge
- **WHEN** an object is selected and the user presses Shift+→
- **THEN** the object moves 10 px right

#### Scenario: Rotate snapping
- **WHEN** the user rotates an object while holding Shift
- **THEN** the angle snaps to multiples of 15° and the chip shows the angle with "Shift: 15° steps"

### Requirement: Select all of a type
The properties sidebar SHALL offer `Select all <type>` to select every object of the current type and edit their shared properties at once. The selected objects SHALL show a thin box each and one box with handles around all of them (mockup 04).

#### Scenario: Recolour all arrows
- **WHEN** the document has 5 arrows and the user uses Select all arrows and picks red
- **THEN** all 5 arrows turn red

### Requirement: Z-order and grouping
Users SHALL change stacking with `Ctrl+]`/`Ctrl+[` (one step past the next unselected object) and `Ctrl+Shift+]`/`Ctrl+Shift+[` (front/back), group with `Ctrl+G` (two or more objects; the group takes the stacking place of the topmost member) and ungroup with `Ctrl+Shift+G` (the members return in place, as the selection). A group SHALL be selected, moved, resized, rotated and stacked as one object, and its selection box SHALL be dashed. Every one of these changes SHALL be one undo step.

#### Scenario: Group moves together
- **WHEN** two objects are grouped and the user drags one
- **THEN** both move together

#### Scenario: Ungroup restores the objects
- **WHEN** the user groups two objects, then ungroups the group
- **THEN** the same two objects are back, unchanged, at the group's place in the stacking order, and selected

#### Scenario: Bring to front
- **WHEN** the bottom object of three is selected and the user presses Ctrl+Shift+]
- **THEN** it is drawn above the other two, and Ctrl+Z puts it back at the bottom

### Requirement: Snapping and guides
While moving, objects SHALL snap with their edges or centre to canvas edges, canvas centre and other objects' edges/centres within 6 screen px, each axis to the nearest line (at equal distance canvas centre before canvas edge before object centre before object edge), showing a 1 px guide line per matching line and a chip naming the target. A dragged resize handle of an unrotated selection SHALL snap the same way. Holding `Alt` SHALL suspend snapping; `Ctrl+;` toggles snapping.

#### Scenario: Centre snap
- **WHEN** the user drags an object near the canvas centre
- **THEN** a guide line appears, the chip says "Canvas centre", and the object snaps to centre

#### Scenario: Alt suspends snapping
- **WHEN** the user drags an object near the canvas centre while holding Alt
- **THEN** the object stays exactly where the pointer puts it and no guide appears

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
Every document change SHALL be undoable with `Ctrl+Z` and redoable with `Ctrl+Shift+Z`, with at least 200 steps per document. A drag (move, resize, rotate) SHALL be one undo step however long it lasts, named after what it did.

#### Scenario: Undo move
- **WHEN** the user moves an object and presses Ctrl+Z
- **THEN** the object returns to its previous position

#### Scenario: One step per drag
- **WHEN** the user drags a group across the canvas in one gesture
- **THEN** a single undo step named "Move Group" returns it to where it started

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
