# Spec Delta

## Purpose
Captures content larger than the screen by scrolling and stitching.

## ADDED Requirements

### Requirement: Manual scrolling capture
After the user selects a region and scrolls manually, the system SHALL capture frames via the ScreenCast portal and SHALL stitch overlapping frames by image matching until the user presses Enter.

#### Scenario: Long page
- **WHEN** the user scrolls through a 5-screen page and presses Enter
- **THEN** one image contains the full page without duplicate or missing rows

### Requirement: Automatic scrolling
Where input injection is allowed (RemoteDesktop portal), the system SHALL scroll automatically and stop at the end of content or at a 20 000 px limit.

#### Scenario: Auto stop
- **WHEN** auto scrolling reaches the end of a page
- **THEN** capture stops and the stitched image opens

### Requirement: Sticky header handling
Fixed headers/footers SHALL be detected and kept only once in the result.

#### Scenario: Sticky header
- **WHEN** a page with a fixed header is captured
- **THEN** the header appears only at the top
