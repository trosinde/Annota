# Spec Delta

## ADDED Requirements

### Requirement: Magnifier
The magnifier SHALL enlarge a source area by 125–800 % into a lens (circle or rounded rectangle) with outline width/colour. The lens SHALL be movable away from its source, drawing an optional connector line.

#### Scenario: Offset lens
- **WHEN** the user drags the lens away from the source area
- **THEN** the lens shows the source area enlarged at its new position with a connector

### Requirement: Spotlight
The spotlight SHALL keep one or more shapes (rectangle, rounded, ellipse, polygon) clear and dim (0–90 %) and/or blur (0–30 px) everything else.

#### Scenario: Two spots
- **WHEN** the user draws two spotlight shapes
- **THEN** both areas are clear and the rest is dimmed

### Requirement: Stamps
The stamp tool SHALL offer searchable categories of original Annota SVG stamps (status marks, numbers, UI pointers, emoji-style faces, keyboard keys) and user stamps imported from PNG/SVG or saved from the current selection.

#### Scenario: Custom stamp
- **WHEN** the user imports company-logo.png as a stamp
- **THEN** it appears under My Stamps and can be placed

### Requirement: Cut-out
The cut-out tool SHALL remove a horizontal or vertical strip and join the remaining parts with an edge style (straight, zigzag, wave, torn).

#### Scenario: Remove middle
- **WHEN** the user drags a horizontal strip and confirms
- **THEN** the image becomes shorter by the strip height with a zigzag seam

### Requirement: Selection with auto-fill
The selection tool SHALL select pixels by rectangle, ellipse, polygon or lasso; moving or deleting the selection SHALL fill the hole with transparency or an auto-fill derived from surrounding pixels.

#### Scenario: Auto-fill
- **WHEN** the user deletes a selected button on a flat background with auto-fill
- **THEN** the hole is filled with the background colour

### Requirement: Fill
The fill tool SHALL flood-fill with tolerance 0–100; `Shift`-click SHALL replace that colour globally.

#### Scenario: Global replace
- **WHEN** the user Shift-clicks a blue pixel with red
- **THEN** all matching blue pixels become red

### Requirement: Eraser and magic wand
The eraser SHALL erase image pixels to the canvas colour with a round or square brush; the magic wand SHALL select contiguous similar pixels with tolerance.

#### Scenario: Wand
- **WHEN** the user clicks a uniform area with the magic wand
- **THEN** that area is selected

### Requirement: Text-only mode
Highlighter, blur and spotlight SHALL offer `Apply to text only`, using text detection to restrict the effect to text regions.

#### Scenario: Highlight text only
- **WHEN** text-only highlight is drawn over a paragraph with icons
- **THEN** only the text lines are highlighted
