# Spec Delta

## MODIFIED Requirements

### Requirement: Cursor inclusion
The user SHALL be able to include or exclude the mouse pointer; default is exclude. For region and screen captures Annota draws the pointer into the image itself, since the Screenshot portal has no pointer option for non-interactive captures; in window mode the desktop's picker decides. A region capture SHALL use the pointer position at the moment the selection is confirmed (`Enter` or the Capture button), and SHALL draw the pointer only when its hot spot lies inside the selection. Confirming with the Capture button puts the pointer on Annota's own toolbar, not over the frozen image, so such a capture contains no pointer.

#### Scenario: Include cursor
- **WHEN** include-cursor is enabled and the user captures the screen
- **THEN** the pointer is visible in the image

#### Scenario: Pointer inside the selection
- **WHEN** include-cursor is enabled, the user has selected a region, moves the pointer inside it and presses `Enter`
- **THEN** the pointer is visible in the image at the position it had when `Enter` was pressed
- **AND** not at the position it had when the overlay opened

#### Scenario: Pointer outside the selection
- **WHEN** include-cursor is enabled and at the moment of confirmation the pointer's hot spot lies outside the selection
- **THEN** no pointer is drawn
- **AND** the captured image is otherwise exactly the selected region

#### Scenario: Pointer on the selection edge
- **WHEN** include-cursor is enabled, the user releases the drag, leaving the pointer on the selection's edge, and presses `Enter` without moving the pointer
- **THEN** the pointer is drawn only if its hot spot lies inside the selection, so a release at the lower-right corner yields no pointer
- **AND** no part of a pointer is silently cut off at the image border

#### Scenario: Delayed region capture places the pointer
- **WHEN** include-cursor is enabled, the user confirms a region with a delay set and moves the pointer into the region during the countdown
- **THEN** the pointer position is taken after the countdown
- **AND** the pointer is visible in the image where it stood when the countdown ended

#### Scenario: Cursor excluded
- **WHEN** include-cursor is disabled
- **THEN** no pointer is drawn in any capture mode
- **AND** the image is the unmodified portal frame for the captured area

## ADDED Requirements

### Requirement: Pointer preview in the region overlay
While include-cursor is enabled, the region overlay SHALL show the user, before confirming, whether the pointer will be part of the image and where it will appear. The indication SHALL update as the selection and the pointer move, and SHALL be absent while include-cursor is disabled.

#### Scenario: Pointer will be included
- **WHEN** include-cursor is enabled and the pointer is inside the current selection
- **THEN** the overlay marks that the pointer will be in the image, at the position it will occupy

#### Scenario: Pointer will not be included
- **WHEN** include-cursor is enabled and the pointer is outside the current selection
- **THEN** the overlay shows that no pointer will be in the image

#### Scenario: Indication follows the selection
- **WHEN** include-cursor is enabled and the user resizes or moves the selection so the pointer changes from inside to outside, or back
- **THEN** the overlay's indication changes with it, without needing a confirmation first

#### Scenario: No indication when excluded
- **WHEN** include-cursor is disabled
- **THEN** the overlay shows no pointer indication
