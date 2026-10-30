# Design: Add scrolling capture

## Mockups (design gate)

Produced by the `designer` agent. Scenario-to-mockup mapping and redlines: [`docs/design/add-scrolling-capture/index.md`](../../../docs/design/add-scrolling-capture/index.md).
Source HTML: `docs/design/add-scrolling-capture/src/`; dark and 2× renders sit next to each light@1x file in `png/`.

- [01-scroll-start](../../../docs/design/add-scrolling-capture/png/01-scroll-start-light@1x.png)
- [02-scroll-capturing-sticky](../../../docs/design/add-scrolling-capture/png/02-scroll-capturing-sticky-light@1x.png)
- [03-auto-scroll-end](../../../docs/design/add-scrolling-capture/png/03-auto-scroll-end-light@1x.png)
- [04-limit-reached](../../../docs/design/add-scrolling-capture/png/04-limit-reached-light@1x.png)
- [05-auto-scroll-unavailable](../../../docs/design/add-scrolling-capture/png/05-auto-scroll-unavailable-light@1x.png)
- [06-stitch-lost-overlap](../../../docs/design/add-scrolling-capture/png/06-stitch-lost-overlap-light@1x.png)
- [07-result-in-editor](../../../docs/design/add-scrolling-capture/png/07-result-in-editor-light@1x.png)
- [08-launcher-scrolling-mode](../../../docs/design/add-scrolling-capture/png/08-launcher-scrolling-mode-light@1x.png)

Design approved: _pending_

## Technical decisions
- Stitching by row-hash matching with fallback to normalized cross-correlation

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
