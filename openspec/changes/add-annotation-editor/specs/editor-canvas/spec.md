# Spec Delta

## Purpose
Hosts the capture and its annotation objects and provides all object manipulation, view and history functions.

## ADDED Requirements

### Requirement: Editor layout
The editor window SHALL show a header bar, a tool strip (left) with the tools whose changes have shipped (so far Select `V` and Crop `C`), a central canvas, a properties sidebar (right) and a tray of open and recent captures (bottom); one window holds several documents. Width ≥ 900 px shows all; narrower windows SHALL show the properties as a bottom sheet over the canvas, hide the tray and move Copy into the main menu. `F9` SHALL show and hide the sidebar (or bottom sheet), kept across sessions.

#### Scenario: Narrow window
- **WHEN** the editor window is resized to 700 px width
- **THEN** the properties panel moves into a bottom sheet, the tray is hidden and Copy is no longer in the header bar
- **AND WHEN** the window is made 1280 px wide again
- **THEN** the properties are back in the sidebar

### Requirement: Header bar
The header bar SHALL show New Capture and linked Undo/Redo at the start; the document name as title and `W × H · Edited` (or `W × H · Not saved` for a new capture, `W × H · 200 %` while not fitted, `Crop · W × H` in crop mode) as subtitle; and Show Properties, Copy, Export as a split button whose menu offers Share, and the main menu at the end.

#### Scenario: Subtitle shows changes
- **WHEN** a document opened from a file is changed
- **THEN** the subtitle reads `W × H · Edited` and the Undo button is enabled

### Requirement: Main menu
The main menu SHALL offer New Capture, Open…, Open Library | Save, Save As…, Export… | Resize Image…, Rotate Right, Rotate Left, Flip Horizontally, Flip Vertically, Flatten All | Preferences, Keyboard Shortcuts, About Annota, with the shortcuts of mockup 17. Open… SHALL open images and `.annota` documents in the editor. Save, Save As…, Export… and Open Library SHALL say in a toast that they arrive with milestone 0.1 until their changes ship.

#### Scenario: Image operations from the menu
- **WHEN** a document is open and the user presses Ctrl+R
- **THEN** the image, the canvas and every annotation turn a quarter clockwise, as one undo step named "Rotate Right"

#### Scenario: Save before add-export-share
- **WHEN** the user picks Save
- **THEN** a toast says saving arrives with milestone 0.1 and the document is unchanged

### Requirement: Context menu
Right-click, `Shift+F10` and the Menu key SHALL open a context menu — at the pointer, without an arrow — with Copy, Duplicate, Delete | Bring to Front, Bring Forward, Send Backward, Send to Back | Group, Ungroup | Flatten Selection, Flatten All and the shortcuts of mockup 05. Right-clicking an object that is not selected SHALL select it first. Entries that cannot apply SHALL be disabled (Ungroup unless a group is selected, Group with fewer than two objects).

#### Scenario: Context menu on a selection
- **WHEN** the user right-clicks an object that is not selected
- **THEN** that object becomes the selection and the menu opens at the pointer with Ungroup disabled

#### Scenario: Group from the context menu
- **WHEN** two objects are selected and the user picks Group in the context menu
- **THEN** they become one group, as one undo step named "Group"

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

### Requirement: Resizing boxes with turned members
When an object inside a group, or inside a selection of several objects, is turned against the box by an angle that is not a multiple of 90°, resizing the box — by a handle or the Arrange entries — SHALL scale it uniformly, by the factor of the side that was dragged or entered, so that the box ends up exactly as dragged.

#### Scenario: Resize a box with a turned member
- **WHEN** two objects are selected, one of them turned by 45°, and the user drags the right handle of their box 200 px to the right
- **THEN** the box's right edge is under the pointer, its height grows by the same factor as its width, and dragging the handle back restores both objects exactly

### Requirement: Arrange entries
The sidebar's Arrange entries (X, Y, W, H, Rotation) SHALL place the selection. They show values rounded to 0.1 px; confirming or leaving an entry whose text was not changed SHALL NOT change the document.

#### Scenario: Unchanged Arrange fields
- **WHEN** an object at a fractional position is selected and the user tabs through X, Y, W, H and Rotation without typing
- **THEN** the document is unchanged and no undo step is added

### Requirement: Select all of a type
The properties sidebar SHALL offer `Select all <type>` to select every object of the current type and edit their shared properties at once. The selected objects SHALL show a thin box each and one box with handles around all of them (mockup 04). A document with more than 40 objects SHALL also list its object types with their counts and a Select button each when nothing is selected (mockup 16).

#### Scenario: Recolour all arrows
- **WHEN** the document has 5 arrows and the user uses Select all arrows and picks red
- **THEN** all 5 arrows turn red

#### Scenario: Select all images
- **WHEN** an image object is selected and the user uses Select All Images
- **THEN** every image object is selected and the sidebar heading reads "N Images"

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

### Requirement: Snapping resize handles
A dragged resize handle SHALL snap by where it lands once the object has kept its aspect ratio (images without Shift, uniformly scaled boxes), not by where the pointer is. A guide and the chip SHALL only name lines the resized box actually touches.

#### Scenario: Snapping a ratio-locked resize
- **WHEN** the user drags an image's corner handle without Shift so that the pointer is near the canvas edge but the kept aspect ratio puts the corner beyond it
- **THEN** no guide and no "Canvas edge" chip appear, and when instead the kept ratio puts the corner within 6 screen px of the canvas edge, the corner snaps onto it and the guide shows there

### Requirement: Snapping switch
The sidebar's "Snap to guides" switch and `Ctrl+;` SHALL be the same setting, kept across sessions.

#### Scenario: Snapping switch
- **WHEN** the user presses `Ctrl+;` on the canvas
- **THEN** the sidebar's "Snap to guides" switch turns off together with snapping, and turning the switch on turns snapping back on

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

### Requirement: Zoom OSD
A zoom OSD at the bottom right of the canvas SHALL show the zoom level ("Fit · 50 %" while fitted, "200 %" otherwise) between zoom-out and zoom-in buttons; its popover SHALL offer Fit to Window, Fit Width (the canvas width fills the view, never above 100 %), the presets, Zoom In and Zoom Out, with their shortcuts.

#### Scenario: Zoom popover
- **WHEN** the user picks 200 % in the zoom popover
- **THEN** the view is at 200 %, the OSD shows "200 %" and the subtitle `W × H · 200 %`, and Fit to Window returns to "Fit · N %"

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
Every document change SHALL be undoable with `Ctrl+Z` and redoable with `Ctrl+Shift+Z`, with at least 200 steps per document. A drag (move, resize, rotate) SHALL be one undo step however long it lasts, named after what it did. The Undo and Redo buttons SHALL be disabled when there is nothing to undo or redo, and their tooltips SHALL name the step ("Undo Move Group").

#### Scenario: Undo move
- **WHEN** the user moves an object and presses Ctrl+Z
- **THEN** the object returns to its previous position

#### Scenario: One step per drag
- **WHEN** the user drags a group across the canvas in one gesture
- **THEN** a single undo step named "Move Group" returns it to where it started

### Requirement: Image operations
Users SHALL crop, resize, rotate by 90° and flip the image, and round its corners. Each SHALL be one undo step, and annotations SHALL keep their place on the image.

#### Scenario: Crop
- **WHEN** the user selects the crop tool, drags the crop handles and presses Enter
- **THEN** the image and canvas are cropped and annotations keep their image positions

### Requirement: Image operations with objects of a newer version
While a document contains objects of a type this version does not know, Crop, Rotate Left and Right, Flip Horizontally and Vertically, Resize Image, Flatten and Flatten All SHALL be unavailable, since those objects could not follow: disabled in the menus, the sidebar and the tool strip, with a tooltip saying that a newer Annota is needed, and their keys SHALL show a toast saying so. All other editing SHALL stay available.

#### Scenario: Document with objects of a newer version
- **WHEN** a document with an object of an unknown type is open and the user presses Ctrl+R
- **THEN** nothing changes, a toast says to update Annota, and Rotate, Flip, Resize Image, Flatten All and the Crop tool are disabled while moving the other objects still works

#### Scenario: Operations return without such objects
- **WHEN** the current document has no objects of an unknown type
- **THEN** crop, rotate, flip, resize image and flatten are available

### Requirement: Crop mode
Crop SHALL be a mode (tool `C`, mockup 08): a frame over the canvas — at first the whole canvas — with corner and edge grips, the canvas outside it dimmed, a rule-of-thirds grid and a "W × H px" chip below it. Dragging inside the frame moves it; it never leaves the canvas. The sidebar SHALL show the frame's X/Y/W/H (editable), an aspect ratio (Free, Original, 1:1, 4:3, 16:9) the frame keeps, Cancel (`Esc`) and Crop (`Enter`).

#### Scenario: Crop cancelled
- **WHEN** the user is in crop mode and presses Esc
- **THEN** crop mode ends, the Select tool is active and the document is unchanged

### Requirement: Crop result
Crop SHALL make the frame the new canvas, keep only the part of the image inside it and re-base the document so the image starts at the origin again; objects outside the new canvas stay in the document. Crop SHALL be unavailable while the frame is the whole canvas or holds no part of the image.

#### Scenario: Nothing to crop
- **WHEN** crop mode starts and the frame is still the whole canvas
- **THEN** Crop is disabled and Enter changes nothing

### Requirement: Resize image
Resize Image… (mockup 09) SHALL offer Width and Height in px or %, Keep aspect ratio (on) and Scale annotations (on). The image is resampled, and the canvas and the objects' positions and sizes scale with it; with Scale annotations, line widths, text sizes, borders and shadows scale too, without it they keep their size.

#### Scenario: Resize with annotations
- **WHEN** the user resizes a 960 × 800 image to 480 × 400 with Scale annotations on
- **THEN** the image is 480 × 400, every annotation keeps its place on the image at half the size, and its line widths are halved

### Requirement: Rotate and flip
Rotate Left and Rotate Right SHALL turn the image, the canvas and every object a quarter turn; Flip Horizontally and Flip Vertically SHALL mirror the image and move every object to its mirrored place with its angle mirrored. Image objects SHALL be mirrored with their picture, like the image; other objects keep their content readable.

#### Scenario: Rotate keeps annotations on the image
- **WHEN** a 1600 × 1000 image with an annotation is rotated right
- **THEN** the canvas is 1000 × 1600, the exported pixels are the old export turned clockwise, and the annotation is turned with the image

#### Scenario: Flip mirrors an inserted picture
- **WHEN** a document with an inserted image object is flipped horizontally
- **THEN** the exported pixels are the old export mirrored left to right, the inserted picture included, and flipping again gives back the same document

### Requirement: Rounded corners
The image's corners SHALL be rounded with the sidebar's Corner radius or a single handle at the image's top-left inner corner, offset by the radius, shown when nothing is selected and the pointer is near it; while it is dragged a chip shows "Corner radius N px". The radius is at most half the image's shorter side.

#### Scenario: Rounded corners
- **WHEN** nothing is selected and the user drags the handle at the image's top-left corner 20 px along the diagonal
- **THEN** the chip shows "Corner radius 20 px" and, on release, the image has 20 px rounded corners as one undo step

### Requirement: Canvas extension
Users SHALL extend the canvas beyond the image by dragging its edges. Near an edge a grabber (12 × 36) appears at its midpoint; while it is dragged the edge is an accent line, a dashed line marks the image edge and a chip says how far the canvas reaches beyond it ("+400 px"). An edge SHALL NOT be dragged into the image.

#### Scenario: Extend for callout
- **WHEN** the user drags the right canvas edge 200 px outward
- **THEN** the canvas grows by 200 px filled with the canvas colour

### Requirement: Canvas background
The canvas colour SHALL be set in the sidebar: seven palette colours, Transparent (the default), Paper, a custom colour, or a colour picked from the image with the eyedropper (the next click on the canvas; Esc cancels). Each change is one undo step.

#### Scenario: Background swatch
- **WHEN** the user clicks the Paper swatch
- **THEN** the canvas outside the image is white in the export, as one undo step named "Canvas Background"

### Requirement: Flatten
Users SHALL flatten the selected objects (`Ctrl+T`) or all objects (`Ctrl+Shift+T`, also the sidebar's Flatten All) into the image without a confirmation: as one undo step, announced by a toast ("7 objects flattened into the image") whose Undo reverts it (mockup 18). Without editable objects the sidebar SHALL say "No editable objects" and Flatten All SHALL be disabled.

#### Scenario: Flatten all
- **WHEN** the user presses Ctrl+Shift+T
- **THEN** no editable objects remain and the image shows them burnt in

#### Scenario: Flatten is undoable
- **WHEN** the user flattens all objects and presses Undo in the toast
- **THEN** every object is back, editable, and the image is as before

### Requirement: Flatten result
Flattening SHALL draw the objects into the image exactly as they are exported, including the image's rounded corners (its corner radius becomes 0). Parts of objects on an extended canvas SHALL become part of the image.

#### Scenario: Flattened pixels
- **WHEN** a document is flattened
- **THEN** its export is pixel for pixel the export from before

### Requirement: Recent captures tray
The tray SHALL show thumbnails of the open documents ("Open"; the current one ringed, unsaved ones with a dot) and of the last 50 captures ("Recent"). Clicking an open document switches to it, clicking a recent capture opens it, and dragging any thumbnail onto the canvas inserts its image as an image object (one undo step; an "Image inserted" toast offers Undo).

#### Scenario: Picture in picture
- **WHEN** the user drags a tray thumbnail onto the canvas
- **THEN** the image is inserted as a movable, selected object, and Ctrl+Z removes it again

### Requirement: Recent captures source
Until the capture library exists, "Recent" SHALL list the newest `.annota` and `.png` files of the save folder that are not open — whatever the file type of the open document, and however its path was spelled. Opening a file that is open already SHALL switch to it instead of opening it twice. The tray SHALL be hideable, kept across sessions.

#### Scenario: Recent capture opens
- **WHEN** the save folder holds a capture that is not open and the user clicks its tray thumbnail
- **THEN** it opens as a further document in the editor window and becomes the current one

#### Scenario: Open image is not recent
- **WHEN** a PNG of the save folder is open in the editor
- **THEN** it is not listed under "Recent", and opening it again switches to the open document
