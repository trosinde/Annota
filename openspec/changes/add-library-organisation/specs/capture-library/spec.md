# Spec Delta

## ADDED Requirements

### Requirement: Full-text search
A search field (`Ctrl+F`) SHALL search item name, tags, application name, window title, source URL, text inside annotations (text, callouts) and, when text recognition is available, recognised image text; results update while typing (≤ 150 ms for 10 000 items) and matching terms are highlighted in the preview.

#### Scenario: Find by annotation text
- **WHEN** a capture contains a callout `Click Connect`
- **THEN** searching `connect` finds it

#### Scenario: Find by screen text
- **WHEN** OCR is enabled and a capture shows the word `Invoice`
- **THEN** searching `invoice` finds it

### Requirement: Filters
A filter bar SHALL combine: date range (today, last 7 days, last 30 days, custom), type (image, video, step guide, template), application, website (domain of source URL), tag, collection and `has annotations`; active filters SHALL show as removable chips.

#### Scenario: Combined filter
- **WHEN** the user filters type=image and application=Firefox for the last 7 days
- **THEN** only matching items are shown with three chips

### Requirement: Tags
Users SHALL add and remove free-text tags on one or many items, with autocomplete from existing tags; a sidebar SHALL list all tags with item counts; renaming or deleting a tag SHALL update all items.

#### Scenario: Bulk tag
- **WHEN** five items are selected and tagged `release-notes`
- **THEN** all five carry the tag and the sidebar shows `release-notes 5`

### Requirement: Collections
Users SHALL create collections and add items by drag and drop or context menu; an item MAY belong to several collections; deleting a collection SHALL NOT delete its items.

#### Scenario: Delete collection
- **WHEN** the user deletes a collection with 12 items
- **THEN** the 12 items remain in the library

### Requirement: Favourites
Users SHALL mark items as favourite (star button or `Ctrl+B` in the library); a Favourites entry in the sidebar SHALL list them.

#### Scenario: Favourite
- **WHEN** the user marks an item as favourite
- **THEN** it appears under Favourites

### Requirement: Saved searches
Users SHALL save the current search and filters as a smart collection that updates automatically.

#### Scenario: Smart collection
- **WHEN** a smart collection `tag:bug, last 30 days` exists and a new capture is tagged bug
- **THEN** the capture appears in the smart collection

### Requirement: Import folders
Users SHALL add additional folders to the library (e.g. `~/Pictures/Screenshots` from GNOME's own screenshot tool); their images are indexed in place and not moved.

#### Scenario: GNOME screenshots
- **WHEN** the user adds ~/Pictures/Screenshots
- **THEN** existing GNOME screenshots appear in the library and stay in their folder

### Requirement: Retention and duplicates
Users SHALL configure automatic trashing of items that are untagged, not favourite and not in a collection after N days (off by default), and SHALL see duplicates (identical pixels) with an option to trash all but one.

#### Scenario: Retention
- **WHEN** retention is 30 days
- **THEN** eligible items older than 30 days are moved to trash on startup and a toast reports the count

### Requirement: Metadata stays portable
Tags, collections and favourites SHALL be stored inside each `.annota` file (and in the sidecar for videos and plain images) so that they survive copying the library folder to another machine.

#### Scenario: Move library
- **WHEN** the library folder is copied to a new computer
- **THEN** tags and collections are restored after indexing
