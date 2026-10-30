# Design: Add advanced annotation tools

## Mockups (design gate)

Mockups live in `docs/design/add-advanced-annotation-tools/` and are produced by the `designer` agent
(HTML in `src/`, PNG light + dark at 1× and 2× in `png/`).
Scenario-to-mockup mapping and redlines: [`docs/design/add-advanced-annotation-tools/index.md`](../../../docs/design/add-advanced-annotation-tools/index.md).

Screens (light, 1×):

- [`01-magnifier-offset-lens`](../../../docs/design/add-advanced-annotation-tools/png/01-magnifier-offset-lens-light@1x.png)
- [`02-spotlight-two-spots`](../../../docs/design/add-advanced-annotation-tools/png/02-spotlight-two-spots-light@1x.png)
- [`03-stamp-picker`](../../../docs/design/add-advanced-annotation-tools/png/03-stamp-picker-light@1x.png)
- [`04-stamp-search-keys`](../../../docs/design/add-advanced-annotation-tools/png/04-stamp-search-keys-light@1x.png)
- [`05-stamp-search-empty`](../../../docs/design/add-advanced-annotation-tools/png/05-stamp-search-empty-light@1x.png)
- [`06-stamp-import-dialog`](../../../docs/design/add-advanced-annotation-tools/png/06-stamp-import-dialog-light@1x.png)
- [`07-my-stamps-placed`](../../../docs/design/add-advanced-annotation-tools/png/07-my-stamps-placed-light@1x.png)
- [`08-cut-out-preview`](../../../docs/design/add-advanced-annotation-tools/png/08-cut-out-preview-light@1x.png)
- [`09-cut-out-applied`](../../../docs/design/add-advanced-annotation-tools/png/09-cut-out-applied-light@1x.png)
- [`10-selection-before-delete`](../../../docs/design/add-advanced-annotation-tools/png/10-selection-before-delete-light@1x.png)
- [`11-selection-auto-filled`](../../../docs/design/add-advanced-annotation-tools/png/11-selection-auto-filled-light@1x.png)
- [`12-selection-moved-transparent`](../../../docs/design/add-advanced-annotation-tools/png/12-selection-moved-transparent-light@1x.png)
- [`13-fill-tolerance`](../../../docs/design/add-advanced-annotation-tools/png/13-fill-tolerance-light@1x.png)
- [`14-fill-global-replace`](../../../docs/design/add-advanced-annotation-tools/png/14-fill-global-replace-light@1x.png)
- [`15-eraser`](../../../docs/design/add-advanced-annotation-tools/png/15-eraser-light@1x.png)
- [`16-magic-wand`](../../../docs/design/add-advanced-annotation-tools/png/16-magic-wand-light@1x.png)
- [`17-text-only-highlight`](../../../docs/design/add-advanced-annotation-tools/png/17-text-only-highlight-light@1x.png)
- [`18-text-only-detecting`](../../../docs/design/add-advanced-annotation-tools/png/18-text-only-detecting-light@1x.png)
- [`19-text-only-unavailable`](../../../docs/design/add-advanced-annotation-tools/png/19-text-only-unavailable-light@1x.png)
- [`20-stamp-set-sheet`](../../../docs/design/add-advanced-annotation-tools/png/20-stamp-set-sheet-light@1x.png)

Design approved: _pending_

## Technical decisions
- Text detection for text-only mode reuses the text-recognition engine
- Stamp sets are designer-made, CC-BY-SA-4.0

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
