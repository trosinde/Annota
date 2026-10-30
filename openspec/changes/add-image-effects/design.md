# Design: Add image effects

## Mockups (design gate)

Mockups live in `docs/design/add-image-effects/` and are produced by the `designer` agent
(HTML in `src/`, PNG light + dark at 1× and 2× in `png/`).
Scenario-to-mockup mapping and redlines: [`docs/design/add-image-effects/index.md`](../../../docs/design/add-image-effects/index.md).

Screens (light, 1×):

- [`01-effects-panel-border`](../../../docs/design/add-image-effects/png/01-effects-panel-border-light@1x.png)
- [`02-add-effect-menu`](../../../docs/design/add-image-effects/png/02-add-effect-menu-light@1x.png)
- [`03-effects-empty`](../../../docs/design/add-image-effects/png/03-effects-empty-light@1x.png)
- [`04-edges-torn-bottom`](../../../docs/design/add-image-effects/png/04-edges-torn-bottom-light@1x.png)
- [`05-perspective-shadow-reflection`](../../../docs/design/add-image-effects/png/05-perspective-shadow-reflection-light@1x.png)
- [`06-shadow-page-curl`](../../../docs/design/add-image-effects/png/06-shadow-page-curl-light@1x.png)
- [`07-capture-info-footer`](../../../docs/design/add-image-effects/png/07-capture-info-footer-light@1x.png)
- [`08-capture-info-screen`](../../../docs/design/add-image-effects/png/08-capture-info-screen-light@1x.png)
- [`09-colour-adjustment`](../../../docs/design/add-image-effects/png/09-colour-adjustment-light@1x.png)
- [`10-effect-on-selection`](../../../docs/design/add-image-effects/png/10-effect-on-selection-light@1x.png)
- [`11-watermark`](../../../docs/design/add-image-effects/png/11-watermark-light@1x.png)
- [`12-save-quick-effect`](../../../docs/design/add-image-effects/png/12-save-quick-effect-light@1x.png)
- [`13-quick-effects`](../../../docs/design/add-image-effects/png/13-quick-effects-light@1x.png)
- [`14-auto-apply-after-capture`](../../../docs/design/add-image-effects/png/14-auto-apply-after-capture-light@1x.png)
- [`15-edit-later-reopened`](../../../docs/design/add-image-effects/png/15-edit-later-reopened-light@1x.png)

Design approved: _pending_

## Technical decisions
- Effects are pure functions on cairo surfaces; GPU not required

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
