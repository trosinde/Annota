# Design: Add screen capture (region, window, screen)

## Mockups (design gate)

Mockups live in `docs/design/add-screen-capture/` and are produced by the `designer` agent.
Scenario-to-mockup mapping: `docs/design/add-screen-capture/index.md`.

Design approved: _pending_

## Technical decisions
- `ashpd` for Screenshot portal (non-interactive full-screen grab, then own overlay)
- Overlay is a fullscreen undecorated GtkWindow per monitor showing the frozen image
- HiDPI: capture at device pixels, store scale factor in document metadata
- X11 window picking via overlay hit-test on window geometry from `_NET_CLIENT_LIST_STACKING`

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
