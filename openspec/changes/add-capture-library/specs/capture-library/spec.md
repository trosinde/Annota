# Spec Delta

## Purpose
Keeps every capture findable through a searchable, taggable library.

## ADDED Requirements

### Requirement: Library view
The library SHALL show all captures as a thumbnail grid grouped by day, with sort by date, name and size.

#### Scenario: Open from library
- **WHEN** the user double-clicks a thumbnail
- **THEN** the capture opens in the editor

### Requirement: Search and filter
Users SHALL search by file name, tags, application name, window title and recognised text, and filter by date range, application and type (image/video).

#### Scenario: Search by app
- **WHEN** the user searches `Firefox`
- **THEN** captures taken from Firefox windows are shown

### Requirement: Tags
Users SHALL add and remove free-text tags on one or many captures.

#### Scenario: Bulk tag
- **WHEN** the user tags 5 selected captures `release-notes`
- **THEN** all 5 show the tag

### Requirement: Retention
Users SHALL configure automatic deletion of untagged captures older than N days (off by default); deleted items go to the system trash.

#### Scenario: Cleanup
- **WHEN** retention is 30 days
- **THEN** untagged captures older than 30 days are moved to trash on startup
