# Spec Delta

## Purpose
Stores every capture automatically and lets users browse, open and manage all captures in one place.

## ADDED Requirements

### Requirement: Automatic storage
Every capture SHALL be saved automatically to the library folder (default `~/Pictures/Annota`, configurable) as `.annota` together with its metadata before the editor opens; videos SHALL be stored as their media file plus a sidecar `.annota-meta.json`. Users MAY disable automatic storage, in which case only explicitly saved files appear in the library.

#### Scenario: Capture is kept
- **WHEN** the user takes a capture and closes the editor without saving
- **THEN** the capture is in the library with its annotations

#### Scenario: Storage disabled
- **WHEN** automatic storage is off and the user discards a capture
- **THEN** nothing is written to the library folder

### Requirement: Capture metadata
Each library item SHALL record: capture time, capture type (region, window, screen, clipboard, file, video), pixel size and scale factor, file size, and where available application name, window title and source URL, plus last-modified time.

#### Scenario: Window capture metadata
- **WHEN** the user captures a Files window
- **THEN** the item shows application `Files`, the window title and the pixel size

### Requirement: Library window
The library SHALL open with `Ctrl+L` from any Annota window and from the app menu, and SHALL show items as a thumbnail grid (default) or list, grouped by day with headings (Today, Yesterday, weekday, date), newest first. Thumbnail size SHALL be adjustable with `Ctrl+scroll` or a zoom slider (3 steps).

#### Scenario: Grouped grid
- **WHEN** the library contains captures from today and last week
- **THEN** they appear under the headings Today and the respective date

#### Scenario: List view
- **WHEN** the user switches to list view
- **THEN** items show thumbnail, name, date, application, size and type in sortable columns

### Requirement: Sorting
Users SHALL sort by capture date, modified date, name, file size and application, ascending or descending; the choice SHALL persist.

#### Scenario: Sort by size
- **WHEN** the user sorts by file size descending
- **THEN** the largest item is first and the setting is kept after restart

### Requirement: Preview and details
Selecting an item SHALL show a preview pane with a large preview (videos: playable) and all metadata; `Space` SHALL toggle a full-window quick look.

#### Scenario: Quick look
- **WHEN** an item is selected and the user presses Space
- **THEN** the item is shown full-window and Space closes it again

### Requirement: Item actions
Users SHALL open an item in the editor (double-click, `Enter`), rename (`F2`), duplicate, copy the flattened image (`Ctrl+C`), drag it out as a flattened PNG, show it in Files, and move it to the system trash (`Del`) with an undo toast.

#### Scenario: Delete with undo
- **WHEN** the user deletes an item and clicks Undo in the toast
- **THEN** the item is restored from the trash

#### Scenario: Drag out
- **WHEN** the user drags an item into a chat window
- **THEN** a flattened PNG is dropped

### Requirement: Multi-select and bulk actions
Users SHALL select multiple items (Ctrl/Shift-click, rubber band, `Ctrl+A`) and apply open, export (with the batch export dialog), copy to folder and trash to all of them.

#### Scenario: Bulk export
- **WHEN** five items are selected and exported as JPG
- **THEN** five JPG files are written with the chosen pattern

### Requirement: Folder synchronisation
The library SHALL reflect files added, renamed or deleted in the library folder by other programs within 2 seconds while Annota runs, and on next start otherwise. Plain PNG/JPG/WebP files in the folder SHALL appear as items.

#### Scenario: External delete
- **WHEN** the user deletes a capture in the Files app
- **THEN** it disappears from the library within 2 seconds

### Requirement: Empty and large libraries
An empty library SHALL show a status page explaining how to capture (with the configured hotkey). With 10 000 items the library SHALL open in under 1 second and scroll at 60 fps.

#### Scenario: Empty state
- **WHEN** the library folder is empty
- **THEN** a status page shows the capture hotkey and a Take capture button

#### Scenario: Large library
- **WHEN** the library holds 10 000 items
- **THEN** it opens in under 1 second
