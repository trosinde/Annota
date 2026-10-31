# Spec Delta

## MODIFIED Requirements

### Requirement: Redaction export warning
Before a document that contains blur or pixelate redactions leaves Annota, the system SHALL warn that these may be reversible and offer to convert them to solid (decision D7; add-export-share mockup 10). Redactions count at any depth, members of groups included. The alert "Make Redactions Solid?" SHALL name how many redactions use blur or pixelate and offer Cancel, a button that continues as it is ("Export Anyway"; "Save Anyway" for quick save and Save As) and Convert to Solid (suggested). Convert to Solid SHALL turn every blur and pixelate redaction into a solid one as one undo step and then continue; Cancel SHALL write nothing and leave the document unchanged; continuing as it is SHALL leave the document unchanged. The warning applies to every way out of Annota: export, quick save, Save As, copy, drag and share. A document without blur or pixelate redactions SHALL continue without it.

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

#### Scenario: Warning before quick save
- **WHEN** a document with a blur redaction is saved with Ctrl+S
- **THEN** the alert says "1 redaction uses blur" and offers Save Anyway instead of Export Anyway

#### Scenario: Only solid redactions
- **WHEN** a document whose redactions are all solid is exported or saved
- **THEN** no alert is shown
