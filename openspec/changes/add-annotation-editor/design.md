# Design: Add annotation editor canvas and document format

## Mockups (design gate)

Mockups live in `docs/design/add-annotation-editor/` (HTML in `src/`, PNG light/dark at 1× and 2× in `png/`).
Scenario-to-mockup mapping and redlines: [docs/design/add-annotation-editor/index.md](../../../docs/design/add-annotation-editor/index.md).

Light 1× renders:

- [01-editor-overview](../../../docs/design/add-annotation-editor/png/01-editor-overview-light@1x.png)
- [02-move-snap-centre](../../../docs/design/add-annotation-editor/png/02-move-snap-centre-light@1x.png)
- [03-rotate-shift-snap](../../../docs/design/add-annotation-editor/png/03-rotate-shift-snap-light@1x.png)
- [04-select-all-arrows](../../../docs/design/add-annotation-editor/png/04-select-all-arrows-light@1x.png)
- [05-context-menu](../../../docs/design/add-annotation-editor/png/05-context-menu-light@1x.png)
- [06-group-and-undo](../../../docs/design/add-annotation-editor/png/06-group-and-undo-light@1x.png)
- [07-zoom-levels](../../../docs/design/add-annotation-editor/png/07-zoom-levels-light@1x.png)
- [08-crop-mode](../../../docs/design/add-annotation-editor/png/08-crop-mode-light@1x.png)
- [09-cropped-resize-dialog](../../../docs/design/add-annotation-editor/png/09-cropped-resize-dialog-light@1x.png)
- [10-canvas-extension-transparent](../../../docs/design/add-annotation-editor/png/10-canvas-extension-transparent-light@1x.png)
- [11-tray-drag-to-canvas](../../../docs/design/add-annotation-editor/png/11-tray-drag-to-canvas-light@1x.png)
- [12-inserted-image-object](../../../docs/design/add-annotation-editor/png/12-inserted-image-object-light@1x.png)
- [13-narrow-bottom-sheet](../../../docs/design/add-annotation-editor/png/13-narrow-bottom-sheet-light@1x.png)
- [14-crash-recovery](../../../docs/design/add-annotation-editor/png/14-crash-recovery-light@1x.png)
- [15-newer-format-error](../../../docs/design/add-annotation-editor/png/15-newer-format-error-light@1x.png)
- [16-many-objects](../../../docs/design/add-annotation-editor/png/16-many-objects-light@1x.png)
- [17-main-menu](../../../docs/design/add-annotation-editor/png/17-main-menu-light@1x.png)
- [18-flatten-all-result](../../../docs/design/add-annotation-editor/png/18-flatten-all-result-light@1x.png)

Design approved: _pending_

## Technical decisions
- Scene graph of `Annotation` trait objects, serde-serialised
- Command pattern for undo/redo
- Cairo renderer shared by canvas and export; canvas uses GtkDrawingArea with damage-region redraws
- Spatial index (R-tree, `rstar`) for hit-testing beyond 100 objects

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
