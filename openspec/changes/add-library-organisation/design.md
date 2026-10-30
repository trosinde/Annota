# Design: Add library organisation (search, tags, collections, filters)

## Mockups (design gate)

Mockups live in `docs/design/add-library-organisation/` (HTML sources in `src/`, renders in `png/` — light and dark, 1× and 2×).
Scenario-to-mockup mapping and redlines: [docs/design/add-library-organisation/index.md](../../../docs/design/add-library-organisation/index.md).

Light 1× renders (dark variants: same name with `-dark@1x.png`):

- [01-library-organised](../../../docs/design/add-library-organisation/png/01-library-organised-light@1x.png)
- [02-search-annotation-text](../../../docs/design/add-library-organisation/png/02-search-annotation-text-light@1x.png)
- [03-search-screen-text](../../../docs/design/add-library-organisation/png/03-search-screen-text-light@1x.png)
- [04-filters-combined](../../../docs/design/add-library-organisation/png/04-filters-combined-light@1x.png)
- [05-filter-add-popover](../../../docs/design/add-library-organisation/png/05-filter-add-popover-light@1x.png)
- [06-filter-date-range](../../../docs/design/add-library-organisation/png/06-filter-date-range-light@1x.png)
- [07-bulk-tag-autocomplete](../../../docs/design/add-library-organisation/png/07-bulk-tag-autocomplete-light@1x.png)
- [08-tag-view](../../../docs/design/add-library-organisation/png/08-tag-view-light@1x.png)
- [09-tag-menu](../../../docs/design/add-library-organisation/png/09-tag-menu-light@1x.png)
- [10-tag-rename-dialog](../../../docs/design/add-library-organisation/png/10-tag-rename-dialog-light@1x.png)
- [11-drag-into-collection](../../../docs/design/add-library-organisation/png/11-drag-into-collection-light@1x.png)
- [12-collection-view](../../../docs/design/add-library-organisation/png/12-collection-view-light@1x.png)
- [13-delete-collection](../../../docs/design/add-library-organisation/png/13-delete-collection-light@1x.png)
- [14-collection-deleted](../../../docs/design/add-library-organisation/png/14-collection-deleted-light@1x.png)
- [15-favourites](../../../docs/design/add-library-organisation/png/15-favourites-light@1x.png)
- [16-favourite-star](../../../docs/design/add-library-organisation/png/16-favourite-star-light@1x.png)
- [17-save-search-dialog](../../../docs/design/add-library-organisation/png/17-save-search-dialog-light@1x.png)
- [18-smart-collection](../../../docs/design/add-library-organisation/png/18-smart-collection-light@1x.png)
- [19-preferences-folders-retention](../../../docs/design/add-library-organisation/png/19-preferences-folders-retention-light@1x.png)
- [20-add-folder-chooser](../../../docs/design/add-library-organisation/png/20-add-folder-chooser-light@1x.png)
- [21-imported-folder](../../../docs/design/add-library-organisation/png/21-imported-folder-light@1x.png)
- [22-retention-toast](../../../docs/design/add-library-organisation/png/22-retention-toast-light@1x.png)
- [23-duplicates-dialog](../../../docs/design/add-library-organisation/png/23-duplicates-dialog-light@1x.png)
- [24-moved-library-indexing](../../../docs/design/add-library-organisation/png/24-moved-library-indexing-light@1x.png)
- [25-search-no-results](../../../docs/design/add-library-organisation/png/25-search-no-results-light@1x.png)
- [26-search-ocr-unavailable](../../../docs/design/add-library-organisation/png/26-search-ocr-unavailable-light@1x.png)
- [27-filter-type-step-guides](../../../docs/design/add-library-organisation/png/27-filter-type-step-guides-light@1x.png)
- [28-narrow-search](../../../docs/design/add-library-organisation/png/28-narrow-search-light@1x.png)
- [29-narrow-tag-sheet](../../../docs/design/add-library-organisation/png/29-narrow-tag-sheet-light@1x.png)

Design approved: _pending_

## Technical decisions
- SQLite FTS5 for full-text search; OCR text indexed in background with low priority
- Collections stored as item membership in the item metadata plus an index table

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
