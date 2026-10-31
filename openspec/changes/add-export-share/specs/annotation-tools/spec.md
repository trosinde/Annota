# Spec Delta

## MODIFIED Requirements

### Requirement: Redaction export warning
Before a document that contains blur or pixelate redactions is exported, copied, dragged out or shared, the system SHALL warn that these may be reversible and offer to convert them to solid (decision D7; add-export-share mockup 10). Redactions count at any depth, members of groups included. The alert "Make Redactions Solid?" SHALL name how many redactions use blur or pixelate and offer Cancel, a button that continues as it is ("Export Anyway", "Copy Anyway", etc., named after the way out) and Convert to Solid (suggested). Convert to Solid SHALL turn every blur and pixelate redaction into a solid one as one undo step and then continue; Cancel SHALL write nothing and leave the document unchanged; continuing as it is SHALL leave the document unchanged. The warning does not apply to saving the document as `.annota` (quick save, Save As): that format keeps the original image under every redaction regardless of this choice, so the warning would not be true there. A document without blur or pixelate redactions SHALL continue without the warning.

#### Scenario: Warning before export
- **WHEN** a document with one pixelate redaction at the top level and one inside a group is exported with Ctrl+E
- **THEN** "Make Redactions Solid?" says "2 redactions use pixelate" before the export dialog opens

#### Scenario: Convert to solid
- **WHEN** the user chooses Convert to Solid in that alert
- **THEN** both redactions, the grouped one included, become solid and the export continues
- **AND** a single Undo makes both pixelate again

#### Scenario: Cancel
- **WHEN** the user chooses Cancel in that alert
- **THEN** nothing is exported and the document is unchanged

#### Scenario: Warning before copy
- **WHEN** a document with a blur redaction is copied with Ctrl+C
- **THEN** "Make Redactions Solid?" offers Cancel, Copy Anyway and Convert to Solid before anything is put on the clipboard

#### Scenario: Warning before a drag
- **WHEN** the user starts dragging the title area of a document with a pixelate redaction
- **THEN** no drag starts and "Make Redactions Solid?" offers Cancel, Drag Anyway and Convert to Solid
- **AND** after Drag Anyway or Convert to Solid, the next drag of the unchanged document hands the image over without asking again

#### Scenario: Warning before share
- **WHEN** a document with a blur redaction is shared with Email…, Open With… or Copy to Folder…
- **THEN** "Make Redactions Solid?" offers Cancel, Share Anyway and Convert to Solid, and Cancel shares nothing

#### Scenario: No warning before a save
- **WHEN** a document with a blur redaction is saved with Ctrl+S or Ctrl+Shift+S
- **THEN** no alert is shown and the document is saved as it is, redactions unconverted

#### Scenario: Only solid redactions
- **WHEN** a document whose redactions are all solid is exported, copied, dragged out or shared
- **THEN** no alert is shown
