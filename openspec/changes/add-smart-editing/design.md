# Design: Add smart move and simplify

## Mockups (design gate)

Mockups live in `docs/design/add-smart-editing/` and are produced by the `designer` agent.
Scenario-to-mockup mapping: `docs/design/add-smart-editing/index.md`.

Design approved: _pending_

## Technical decisions
- OpenCV (`opencv` crate) for contour detection; heavy work in a worker thread

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
