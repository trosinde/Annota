# Design: Add smart move and simplify

## Mockups (design gate)

Mockups live in `docs/design/add-smart-editing/` and are produced by the `designer` agent
(HTML in `src/`, PNG light + dark at 1× and 2× in `png/`).
Scenario-to-mockup mapping and redlines: [`docs/design/add-smart-editing/index.md`](../../../docs/design/add-smart-editing/index.md).

Screens (light, 1×):

- [`01-smart-move-analysing`](../../../docs/design/add-smart-editing/png/01-smart-move-analysing-light@1x.png)
- [`02-smart-move-detected`](../../../docs/design/add-smart-editing/png/02-smart-move-detected-light@1x.png)
- [`03-smart-move-low-detail`](../../../docs/design/add-smart-editing/png/03-smart-move-low-detail-light@1x.png)
- [`04-smart-move-dragging`](../../../docs/design/add-smart-editing/png/04-smart-move-dragging-light@1x.png)
- [`05-smart-move-result`](../../../docs/design/add-smart-editing/png/05-smart-move-result-light@1x.png)
- [`06-simplify-before-after`](../../../docs/design/add-smart-editing/png/06-simplify-before-after-light@1x.png)
- [`07-simplify-applied-coral`](../../../docs/design/add-smart-editing/png/07-simplify-applied-coral-light@1x.png)
- [`08-simplify-low-detail-ink`](../../../docs/design/add-smart-editing/png/08-simplify-low-detail-ink-light@1x.png)
- [`09-simplify-new-palette`](../../../docs/design/add-smart-editing/png/09-simplify-new-palette-light@1x.png)
- [`10-main-menu-entry`](../../../docs/design/add-smart-editing/png/10-main-menu-entry-light@1x.png)

Design approved: _pending_

## Technical decisions
- OpenCV (`opencv` crate) for contour detection; heavy work in a worker thread

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
