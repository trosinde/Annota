# Design: Add image effects

## Mockups (design gate)

Mockups live in `docs/design/add-image-effects/` and are produced by the `designer` agent.
Scenario-to-mockup mapping: `docs/design/add-image-effects/index.md`.

Design approved: _pending_

## Technical decisions
- Effects are pure functions on cairo surfaces; GPU not required

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
