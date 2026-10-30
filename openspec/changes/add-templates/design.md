# Design: Add templates and image combining

## Mockups (design gate)

Mockups live in `docs/design/add-templates/` (HTML sources in `src/`, renders in `png/` — light and dark, 1× and 2×).
Scenario-to-mockup mapping and redlines: [docs/design/add-templates/index.md](../../../docs/design/add-templates/index.md).

Light 1× renders (dark variants: same name with `-dark@1x.png`):

- [01-new-from-template-menu](../../../docs/design/add-templates/png/01-new-from-template-menu-light@1x.png)
- [02-template-chooser](../../../docs/design/add-templates/png/02-template-chooser-light@1x.png)
- [03-empty-step-guide](../../../docs/design/add-templates/png/03-empty-step-guide-light@1x.png)
- [04-tray-select-combine](../../../docs/design/add-templates/png/04-tray-select-combine-light@1x.png)
- [05-combine-dialog](../../../docs/design/add-templates/png/05-combine-dialog-light@1x.png)
- [06-combine-reorder](../../../docs/design/add-templates/png/06-combine-reorder-light@1x.png)
- [07-step-guide-filled](../../../docs/design/add-templates/png/07-step-guide-filled-light@1x.png)
- [08-slot-drop-target](../../../docs/design/add-templates/png/08-slot-drop-target-light@1x.png)
- [09-slot-filled-fit](../../../docs/design/add-templates/png/09-slot-filled-fit-light@1x.png)
- [10-save-as-template](../../../docs/design/add-templates/png/10-save-as-template-light@1x.png)
- [11-chooser-my-templates](../../../docs/design/add-templates/png/11-chooser-my-templates-light@1x.png)
- [12-before-after-filled](../../../docs/design/add-templates/png/12-before-after-filled-light@1x.png)
- [13-timeline-filled](../../../docs/design/add-templates/png/13-timeline-filled-light@1x.png)
- [14-narrow-chooser](../../../docs/design/add-templates/png/14-narrow-chooser-light@1x.png)

Design approved: _pending_

## Technical decisions
- Templates are `.annota` documents with slot objects and theme tokens

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
