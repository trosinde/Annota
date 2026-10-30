# Spec Delta

## Purpose
Turns a recorded click sequence into an editable step-by-step guide.

## ADDED Requirements

### Requirement: Record steps
During step capture, each mouse click SHALL produce a capture of the active window with a step marker at the click position; `Pause` and `Stop` are available in a floating control.

#### Scenario: Three clicks
- **WHEN** the user clicks three times during step capture
- **THEN** three numbered steps are created

### Requirement: Step descriptions
Each step SHALL get a default description from accessibility information of the clicked element where available (e.g. `Click "Save"`), editable by the user.

#### Scenario: Button name
- **WHEN** the user clicks a button labelled Save
- **THEN** the step text reads Click "Save"

### Requirement: Guide output
Steps SHALL be editable in a step list and exportable as step-guide template document, PDF and Markdown with images.

#### Scenario: Markdown
- **WHEN** the user exports as Markdown
- **THEN** a .md file and an images folder are written
