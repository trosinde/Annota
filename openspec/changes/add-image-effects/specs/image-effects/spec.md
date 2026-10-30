# Spec Delta

## Purpose
Applies non-destructive visual effects to a capture or a selected part of it.

## ADDED Requirements

### Requirement: Non-destructive effects
Effects SHALL be stored as an ordered, editable effect stack on the document (or on a selection) and rendered at export; each effect can be toggled and edited later.

#### Scenario: Edit later
- **WHEN** the user reopens a saved document with a border effect
- **THEN** the border width can still be changed

### Requirement: Effect catalogue
The system SHALL provide: border (colour, width, radius), edges (torn, wave, zigzag, fade; sides; size), shadow (colour, offset, blur, opacity), perspective (tilt, rotation), page curl (corner), reflection (size, opacity), colour adjustment (brightness, contrast, hue, saturation, gamma), filters (grayscale, sepia, invert, sharpen, soften), colour replacement (from, to, tolerance) and watermark (image, position, scale, opacity, overlay/emboss).

#### Scenario: Torn bottom edge
- **WHEN** the user applies edges=torn to the bottom side only
- **THEN** only the bottom edge is torn

### Requirement: Capture info
The capture-info effect SHALL add a footer with any of: date/time, application name, window title, OS name/version, image size, custom text; fields come from capture metadata.

#### Scenario: Footer
- **WHEN** capture info with date and application is applied to a window capture
- **THEN** a footer shows the capture time and the application name

### Requirement: Quick effects
Users SHALL save the current effect stack as a named quick effect and apply it with one click or automatically after every capture.

#### Scenario: Auto-apply
- **WHEN** a quick effect is set to apply after capture
- **THEN** every new capture opens with that effect stack
