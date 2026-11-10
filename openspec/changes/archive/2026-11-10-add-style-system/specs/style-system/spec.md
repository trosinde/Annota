# Spec Delta

## Purpose
Lets users apply, save and share consistent visual styles across all annotation tools.

## ADDED Requirements

### Requirement: Quick styles
Each drawing tool SHALL show a grid of quick styles in the properties sidebar: the current theme's eight generated styles, then the user's own styles ("My Styles"). A quick style SHALL be a named set of the tool's style properties: the properties the sidebar edits, except a text's words, a step marker's own value, a shape's kind and how a step sequence counts.

#### Scenario: Default style
- **WHEN** a drawing tool is chosen with the default theme and its defaults unchanged
- **THEN** its eight theme styles are shown and the first, equal to the tool's defaults, is selected

### Requirement: Applying quick styles
Choosing a quick style SHALL apply all its properties where the sidebar's properties go: to the selected objects as one undo step (none if the style is already in use), to the text being edited, or, with nothing selected, to the drawing tool's defaults.

#### Scenario: Apply style to a selection
- **WHEN** the user picks a quick style with two arrows selected
- **THEN** both arrows take all of the style's properties as one undo step, and picking it again adds no step

### Requirement: Saving quick styles
When the current values match no quick style the sidebar SHALL say "Modified" and offer `Save as Quick Style` with a suggested name; otherwise saving SHALL be unavailable. A saved style SHALL be added at the end of the tool's own styles, selected, and the save SHALL offer Undo in a toast.

#### Scenario: Save style
- **WHEN** the user changes arrow colour and width and clicks Save as Quick Style
- **THEN** the new style appears at the end of the arrow grid, selected

### Requirement: Managing quick styles
The user's own styles SHALL be renamable, movable left and right among the tool's styles, updatable from the current properties and deletable, from a context menu (secondary click, long press, Menu key, Shift+F10) and with F2, Ctrl+←/→ and Delete on a focused tile; Enter SHALL apply it. Deleting SHALL ask no confirmation and offer Undo in a toast. Theme styles SHALL only be applied and starred.

#### Scenario: Delete with undo
- **WHEN** the user presses Delete on a focused style of their own
- **THEN** the style is removed, the selected objects stay, and the toast's Undo puts the style back at its place

#### Scenario: Reorder with the keyboard
- **WHEN** the user presses Ctrl+← on their second arrow style
- **THEN** it becomes the first, and Move Left is unavailable for it

### Requirement: Quick style persistence
The user's styles, favourites and current theme SHALL persist across runs in a versioned file. Data of a newer minor version that this version does not understand SHALL be kept and written back; a file of a newer major version SHALL neither be read nor overwritten.

#### Scenario: Newer library file
- **WHEN** the library file was written by a newer major version of Annota
- **THEN** Annota starts with no user styles and leaves the file unchanged

### Requirement: Favourites
Users SHALL star quick styles, their own and the theme's; a Favourites view SHALL list starred styles across all tools grouped by tool, and choosing one SHALL make its tool active with that style. With nothing starred the view SHALL show only a hint.

#### Scenario: Use favourite
- **WHEN** the user picks a starred callout style from Favourites
- **THEN** the callout tool becomes active with that style

#### Scenario: No favourites
- **WHEN** no style is starred and the user opens Favourites
- **THEN** only the hint is shown

### Requirement: Themes
A theme SHALL consist of 2–8 colours, a heading font and a body font; the default theme is `Annota`. Selecting a theme SHALL regenerate theme-based quick styles for every tool, deterministically from its colours and fonts, make each tool's first theme style its defaults and use the theme's colours as the sidebar's palette swatches. User styles SHALL stay unchanged.

#### Scenario: Apply theme
- **WHEN** the user selects a theme with brand colours
- **THEN** every tool's theme quick styles use those colours, and new objects of every tool take the theme's first colour

### Requirement: Theme editor
The user SHALL be able to create, duplicate, rename, recolour, reorder, refont and delete their own themes in a theme editor with a live preview of generated styles; deleting SHALL be confirmed. Built-in and managed themes SHALL not be editable.

#### Scenario: Colour count
- **WHEN** the user edits a theme of their own
- **THEN** a colour can be added only below 8 colours and removed only above 2

### Requirement: Theme exchange
Themes SHALL be exportable and importable as `.annota-theme` (JSON) files with a format version. Files of a newer minor version SHALL be read with their unknown data kept; files of an unknown major version SHALL be refused.

#### Scenario: Round trip
- **WHEN** the user exports a theme and imports the file
- **THEN** the imported theme has the same colours and fonts

### Requirement: Theme import errors
An import that fails SHALL name the reason in an alert — not a JSON file, an unknown or missing schema version, a colour count outside 2–8, an invalid colour, no name, no fonts — and add no theme.

#### Scenario: Import a theme with too many colours
- **WHEN** the user imports a theme file with 11 colours
- **THEN** an alert says the file is not a valid Annota theme because it contains 11 colours and a theme needs 2–8, and no theme is added

### Requirement: Managed themes
A system-wide theme directory `/usr/share/annota/themes` SHALL be read for managed deployments. Its themes SHALL be read-only and can be duplicated into the user's own themes.

#### Scenario: Managed theme
- **WHEN** an admin installs a theme file into /usr/share/annota/themes
- **THEN** the theme appears for every user, read-only
