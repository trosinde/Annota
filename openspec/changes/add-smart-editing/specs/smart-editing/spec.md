# Spec Delta

## Purpose
Treats flat screenshots as editable UI layouts.

## ADDED Requirements

### Requirement: Smart move
Activating smart move SHALL detect UI elements and text blocks (edge/contour analysis locally) and convert them into movable objects; vacated areas are filled from surrounding background. A detail slider controls granularity.

#### Scenario: Move button
- **WHEN** smart move is on and the user drags a detected button
- **THEN** the button moves and its old place shows background

### Requirement: Simplify
Simplify SHALL replace text lines with rounded bars and UI elements with flat shapes in theme colours; a detail slider and custom palettes are available; results remain editable objects.

#### Scenario: Simplify screenshot
- **WHEN** the user applies simplify to a settings dialog
- **THEN** text is replaced by bars and the layout is preserved
