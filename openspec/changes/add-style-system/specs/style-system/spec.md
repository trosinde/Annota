# Spec Delta

## Purpose
Lets users apply, save and share consistent visual styles across all annotation tools.

## ADDED Requirements

### Requirement: Quick styles
Each tool SHALL show a grid of quick styles in the properties sidebar. Changing properties SHALL offer `Save as quick style`; user styles can be renamed, reordered and deleted.

#### Scenario: Save style
- **WHEN** the user changes arrow colour and width and clicks Save as quick style
- **THEN** the new style appears at the end of the arrow grid

### Requirement: Favourites
Users SHALL star quick styles; a Favourites view SHALL list starred styles across all tools and switch tool when one is chosen.

#### Scenario: Use favourite
- **WHEN** the user picks a starred callout style from Favourites
- **THEN** the callout tool becomes active with that style

### Requirement: Themes
A theme SHALL consist of 2–8 colours, a heading font and a body font. Selecting a theme SHALL regenerate theme-based quick styles for every tool; the default theme is `Annota`.

#### Scenario: Apply theme
- **WHEN** the user selects a theme with brand colours
- **THEN** every tool's theme quick styles use those colours

### Requirement: Theme exchange
Themes SHALL be exportable and importable as `.annota-theme` (JSON) files; a system-wide theme directory `/usr/share/annota/themes` SHALL be read for managed deployments.

#### Scenario: Managed theme
- **WHEN** an admin installs a theme file into /usr/share/annota/themes
- **THEN** the theme appears for every user, read-only
