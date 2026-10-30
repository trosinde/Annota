# Spec Delta

## Purpose
Arranges multiple captures into consistent, reusable page layouts.

## ADDED Requirements

### Requirement: Built-in templates
The system SHALL ship original templates: step guide (2–8 steps), comparison (2–3 columns), before/after, grid (2×2, 3×2) and timeline.

#### Scenario: New from template
- **WHEN** the user chooses New → From template → Step guide
- **THEN** an empty step guide with placeholders opens

### Requirement: Combine captures
Users SHALL select captures in the tray and choose Combine; a dialog lets them pick a template, order images, enter title and captions and toggle numbering.

#### Scenario: Combine three
- **WHEN** three captures are combined into a step guide
- **THEN** each is placed in a numbered slot with its caption

### Requirement: Placeholders
Template slots SHALL accept images by drag and drop and scale them by fit or fill.

#### Scenario: Drop into slot
- **WHEN** the user drops a capture onto an empty slot
- **THEN** the slot shows the image fitted

### Requirement: Custom templates
Users SHALL save any document as a template with marked slots; it then appears in the template chooser.

#### Scenario: Reuse
- **WHEN** the user saves a layout as template
- **THEN** it is offered under My templates
