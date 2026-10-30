# Design: Add capture library (browse and manage captures)

## Mockups (design gate)

Mockups live in `docs/design/add-capture-library/` (HTML sources in `src/`, renders in `png/` — light and dark, 1× and 2×).
Scenario-to-mockup mapping and redlines: [docs/design/add-capture-library/index.md](../../../docs/design/add-capture-library/index.md).

Light 1× renders (dark variants: same name with `-dark@1x.png`):

- [01-library-grid](../../../docs/design/add-capture-library/png/01-library-grid-light@1x.png)
- [02-library-preview](../../../docs/design/add-capture-library/png/02-library-preview-light@1x.png)
- [03-library-list](../../../docs/design/add-capture-library/png/03-library-list-light@1x.png)
- [04-sort-menu](../../../docs/design/add-capture-library/png/04-sort-menu-light@1x.png)
- [05-thumbnail-zoom-large](../../../docs/design/add-capture-library/png/05-thumbnail-zoom-large-light@1x.png)
- [06-quick-look](../../../docs/design/add-capture-library/png/06-quick-look-light@1x.png)
- [07-multi-select](../../../docs/design/add-capture-library/png/07-multi-select-light@1x.png)
- [08-bulk-export-dialog](../../../docs/design/add-capture-library/png/08-bulk-export-dialog-light@1x.png)
- [09-item-context-menu](../../../docs/design/add-capture-library/png/09-item-context-menu-light@1x.png)
- [10-rename-inline](../../../docs/design/add-capture-library/png/10-rename-inline-light@1x.png)
- [11-delete-undo-toast](../../../docs/design/add-capture-library/png/11-delete-undo-toast-light@1x.png)
- [12-drag-out](../../../docs/design/add-capture-library/png/12-drag-out-light@1x.png)
- [13-empty-state](../../../docs/design/add-capture-library/png/13-empty-state-light@1x.png)
- [14-nas-offline](../../../docs/design/add-capture-library/png/14-nas-offline-light@1x.png)
- [15-nas-back-online](../../../docs/design/add-capture-library/png/15-nas-back-online-light@1x.png)
- [16-folder-sync](../../../docs/design/add-capture-library/png/16-folder-sync-light@1x.png)
- [17-preferences-library](../../../docs/design/add-capture-library/png/17-preferences-library-light@1x.png)
- [18-preferences-storage-off](../../../docs/design/add-capture-library/png/18-preferences-storage-off-light@1x.png)
- [19-editor-tray](../../../docs/design/add-capture-library/png/19-editor-tray-light@1x.png)
- [20-editor-menu-library](../../../docs/design/add-capture-library/png/20-editor-menu-library-light@1x.png)
- [21-narrow-grid](../../../docs/design/add-capture-library/png/21-narrow-grid-light@1x.png)
- [22-narrow-preview](../../../docs/design/add-capture-library/png/22-narrow-preview-light@1x.png)
- [23-narrow-sidebar](../../../docs/design/add-capture-library/png/23-narrow-sidebar-light@1x.png)
- [24-large-library](../../../docs/design/add-capture-library/png/24-large-library-light@1x.png)
- [25-trash](../../../docs/design/add-capture-library/png/25-trash-light@1x.png)
- [26-video-preview](../../../docs/design/add-capture-library/png/26-video-preview-light@1x.png)

Design approved: 2026-10-30

## Technical decisions
- Files stay the source of truth; SQLite index (`rusqlite`) in `~/.cache/annota/library.db` is rebuildable
- Thumbnails cached per XDG thumbnail spec (normal/large) in `~/.cache/thumbnails`
- `GtkGridView`/`GtkColumnView` with lazy thumbnail loading; `notify` crate for folder watching
- Metadata: application/window title via portal response where available, else from X11 properties; URL only where the capture source provides it

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
