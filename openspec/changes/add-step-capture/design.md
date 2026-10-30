# Design: Add step capture (automatic how-to guides)

## Mockups (design gate)

Mockups live in `docs/design/add-step-capture/` and were produced by the `designer` agent.
Scenario-to-mockup mapping and redlines: [`docs/design/add-step-capture/index.md`](../../../docs/design/add-step-capture/index.md).

- [01-step-capture-started](../../../docs/design/add-step-capture/png/01-step-capture-started-light@1x.png)
- [02-step-capture-three-clicks](../../../docs/design/add-step-capture/png/02-step-capture-three-clicks-light@1x.png)
- [03-step-capture-paused](../../../docs/design/add-step-capture/png/03-step-capture-paused-light@1x.png)
- [04-step-list-editor](../../../docs/design/add-step-capture/png/04-step-list-editor-light@1x.png)
- [05-edit-step-description](../../../docs/design/add-step-capture/png/05-edit-step-description-light@1x.png)
- [06-reorder-steps](../../../docs/design/add-step-capture/png/06-reorder-steps-light@1x.png)
- [07-step-deleted-undo](../../../docs/design/add-step-capture/png/07-step-deleted-undo-light@1x.png)
- [08-guide-empty](../../../docs/design/add-step-capture/png/08-guide-empty-light@1x.png)
- [09-export-markdown](../../../docs/design/add-step-capture/png/09-export-markdown-light@1x.png)
- [10-export-pdf-progress](../../../docs/design/add-step-capture/png/10-export-pdf-progress-light@1x.png)
- [11-export-done](../../../docs/design/add-step-capture/png/11-export-done-light@1x.png)
- [12-template-document](../../../docs/design/add-step-capture/png/12-template-document-light@1x.png)

Dark and 2× renders sit next to each file (`-dark@1x.png`, `@2x.png`).

Design approved: _pending_

## Technical decisions
- Click events via AT-SPI on GNOME (accessibility bus); Wayland limitations documented

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
