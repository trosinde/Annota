# Spec Delta

## Purpose
Recognises text in captures locally and lets users copy or replace it.

## ADDED Requirements

### Requirement: Local recognition
Text recognition SHALL run locally with Tesseract (languages: system locale + English, configurable) and SHALL never send image data over the network.

#### Scenario: Offline
- **WHEN** the computer is offline and the user grabs text
- **THEN** recognition succeeds

### Requirement: Grab text
Users SHALL copy recognised text of the whole image or a selection (`Ctrl+Shift+C`), preserving line breaks; a dialog shows the text for review before copying.

#### Scenario: Copy selection
- **WHEN** the user selects a paragraph and presses Ctrl+Shift+C
- **THEN** the paragraph text is shown and copied

### Requirement: Edit text
Users SHALL double-click recognised text to edit it; the original text is covered with the sampled background colour and replaced with a text object matching estimated size and colour.

#### Scenario: Fix typo
- **WHEN** the user edits `Recieve` to `Receive`
- **THEN** the image shows `Receive` in a similar font size and colour

### Requirement: Text capture mode
A capture mode `text` SHALL recognise the selected region and copy the text without opening the editor.

#### Scenario: Direct
- **WHEN** the user runs `annota --capture text` and selects a region
- **THEN** the text is on the clipboard and a notification confirms it
