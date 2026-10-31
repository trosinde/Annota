# Spec Delta

## Purpose
Provides the drawing tools that create annotation objects on the canvas and the properties of each object type.

## ADDED Requirements

### Requirement: Tool switching
Each tool SHALL have a single-key shortcut: Select `V`, Arrow `A`, Line `L`, Pen `P`, Shape `U`, Text `T`, Callout `K`, Step `S`, Highlighter `H`, Redact (blur, pixelate, solid) `B`, Crop `C`. Shortcuts SHALL appear in tooltips. After drawing, the tool stays active until `Esc` or `V`. While text is edited in place, letter keys type text instead of switching tools.

#### Scenario: Key switches tool
- **WHEN** the canvas has focus and the user presses `A`
- **THEN** the arrow tool is active

#### Scenario: Tool stays active
- **WHEN** the user draws an arrow with the arrow tool
- **THEN** the arrow tool is still active, a second arrow can be drawn right away, and `Esc` returns to Select

### Requirement: Common style properties
Every drawable object SHALL support colour (palette, hex/RGB entry, screen eyedropper), opacity 0–100 %, and a drop shadow with on/off, colour, angle, distance, blur and opacity.

#### Scenario: Eyedropper
- **WHEN** the user clicks the eyedropper and then a pixel anywhere on screen
- **THEN** the current object takes that colour

### Requirement: Arrow
Arrows SHALL support width 1–40 px, independent start and end styles (none, triangle, open, circle, square, bar), independent end sizes, line style (solid, dashed, dotted), and a curve handle for a quadratic Bézier. Shift constrains to 0/45/90°.

#### Scenario: Curved arrow
- **WHEN** the user drags the arrow's middle handle
- **THEN** the arrow bends smoothly through the handle

#### Scenario: Double-ended
- **WHEN** the user sets both start and end style to triangle
- **THEN** both ends show arrowheads

### Requirement: Line
Lines SHALL share the arrow properties without default end styles.

#### Scenario: Plain line
- **WHEN** the user draws with the line tool
- **THEN** a line without arrowheads is created

### Requirement: Pen
The pen SHALL draw freehand strokes as editable vector paths with width, colour, opacity and smoothing 0–100; Shift draws straight segments.

#### Scenario: Editable stroke
- **WHEN** the user draws a stroke and later changes its width
- **THEN** the stroke re-renders with the new width

### Requirement: Shape
Shapes SHALL include rectangle, rounded rectangle (corner radius), ellipse and polygon (click per vertex, double-click, Enter or a click on the first vertex to finish, Backspace removes the last vertex, Esc cancels). Fill and outline are independent; fill may be transparent. Shift draws square/circle.

#### Scenario: Frame
- **WHEN** the user draws a rectangle with transparent fill
- **THEN** only the outline is visible

#### Scenario: Polygon
- **WHEN** the shape type is polygon and the user clicks four points and presses Enter
- **THEN** a four-sided polygon is created as one undo step

### Requirement: Text
Text objects SHALL support font family, size, bold, italic, underline, strikethrough, alignment, colour, outline colour/width and multi-line editing in place.

#### Scenario: Edit in place
- **WHEN** the user double-clicks a text object
- **THEN** the text becomes editable on the canvas

### Requirement: Callout
Callouts SHALL be a text box with shape (rectangle, rounded, ellipse, cloud), fill, outline, padding and one or more tails. Dragging the tail-add handle SHALL create an additional tail; dragging a tail tip onto the body SHALL remove it, except the last tail, which stays where it was. The body auto-grows with text.

#### Scenario: Two tails
- **WHEN** the user drags the add-tail handle to a second point
- **THEN** the callout points to two locations

### Requirement: Step
Clicking SHALL place sequential markers. Sequence types: 1,2,3 / A,B,C / a,b,c / I,II,III. Users SHALL restart the sequence, edit a single marker's value, and choose shape (circle, square, rounded square, pin). Deleting a marker renumbers following markers of the same sequence.

#### Scenario: Auto-increment
- **WHEN** the user clicks three times with the step tool
- **THEN** markers 1, 2 and 3 are created

#### Scenario: Renumber on delete
- **WHEN** markers 1–4 exist and the user deletes marker 2
- **THEN** the remaining markers read 1, 2, 3

### Requirement: Highlighter
The highlighter SHALL draw a semi-transparent rectangle (default 45 % yellow) using multiply blending so underlying text stays readable.

#### Scenario: Text readable
- **WHEN** the user highlights a line of text
- **THEN** the text remains legible through the highlight

### Requirement: Redaction
The redact tool SHALL support types blur, pixelate and solid, with intensity and shape (rectangle, ellipse). Blur and pixelate SHALL process the captured image's own pixels under the redaction, so the canvas and the export show the same result.

#### Scenario: Pixelate hides detail
- **WHEN** the user drags a pixelate redaction over text in the capture
- **THEN** the area shows blocks of averaged colour on the canvas and in the export, and the text cannot be read

### Requirement: Redaction export warning
When exporting or sharing a document that contains blur or pixelate redactions, the system SHALL warn that these may be reversible and offer to convert them to solid. The warning applies to every way out of Annota: export, copy, drag and share (decision D7). None of those paths exist yet — `add-export-share` completes this requirement with a `MODIFIED Requirements` delta once they do.

#### Scenario: No way out of Annota yet
- **WHEN** a document contains a blur or pixelate redaction
- **THEN** the redaction renders and is editable like any object
- **AND** no export/copy/drag/share path exists yet to warn on, since `add-export-share` hasn't shipped
