# Spec Delta

## REMOVED Requirements

### Requirement: Redaction export warning
**Reason**: Its only scenario, "No way out of Annota yet", states that no export, copy, drag or share path exists to warn on because this change had not shipped. This change ships them, so the scenario is false from here on. A MODIFIED block replaces the whole requirement and cannot drop a scenario.
**Migration**: Replaced by the ADDED requirement "Reversible redaction warning" below, which carries the full warning behaviour over every way out of Annota (decision D7, mockup 10). A new name because a requirement cannot be both removed and added in one delta.

## ADDED Requirements

### Requirement: Reversible redaction warning
Before a document with blur or pixelate redactions leaves Annota — export, copy, drag out or share — the system SHALL warn that these may be reversible and offer to convert them to solid (D7, mockup 10). Redactions count at any depth, groups included. The alert "Make Redactions Solid?" SHALL name how many are blur or pixelate and offer Cancel, Convert to Solid (suggested) and a button that continues unconverted, named after the way out. Saving as `.annota` is not a way out and SHALL NOT warn.

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

#### Scenario: Continue unconverted
- **WHEN** the user chooses the button that continues as it is ("Export Anyway", or whatever the way out names it)
- **THEN** the export, copy, drag or share continues with the redactions as they are
- **AND** the document is left unchanged

#### Scenario: Warning before copy
- **WHEN** a document with a blur redaction is copied with Ctrl+C, with or without objects selected (Copy with a selection puts the flattened image on the clipboard too)
- **THEN** "Make Redactions Solid?" offers Cancel, Copy Anyway and Convert to Solid before anything is put on the clipboard

#### Scenario: Warning before the Copy of a new capture's toast
- **WHEN** a blur redaction is drawn on a new capture while its toast is shown and the toast's Copy is pressed, with that capture or another document current
- **THEN** "Make Redactions Solid?" offers Cancel, Copy Anyway and Convert to Solid for that capture before anything is put on the clipboard

#### Scenario: Warning before a drag
- **WHEN** the user starts dragging the title area of a document with a pixelate redaction
- **THEN** no drag starts and "Make Redactions Solid?" offers Cancel, Drag Anyway and Convert to Solid
- **AND** after Drag Anyway or Convert to Solid, the next drag of the unchanged document hands the image over without asking again

#### Scenario: Warning before share
- **WHEN** a document with a blur redaction is shared with Email…, Open With… or Copy to Folder…
- **THEN** "Make Redactions Solid?" offers Cancel, Share Anyway and Convert to Solid, and Cancel shares nothing

#### Scenario: Warning before batch export
- **WHEN** three documents are selected in the tray for a batch export and two of them hold one blur and one pixelate redaction
- **THEN** a single "Make Redactions Solid?" says "2 redactions use blur or pixelate" and offers Cancel, Export Anyway and Convert to Solid before "Export 3 Images" opens
- **AND** Convert to Solid converts them in every selected document — in an open document as its own undo step, in a recent file only in the copy that is exported, never in the file

#### Scenario: No warning before a save
- **WHEN** a document with a blur redaction is saved with Ctrl+S or Ctrl+Shift+S
- **THEN** no alert is shown and the document is saved as it is, redactions unconverted

#### Scenario: Only solid redactions
- **WHEN** a document whose redactions are all solid is exported, copied, dragged out or shared
- **THEN** no alert is shown
