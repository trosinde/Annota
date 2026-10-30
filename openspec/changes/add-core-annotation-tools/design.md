# Design: Add core annotation tools

## Mockups (design gate)

Mockups live in `docs/design/add-core-annotation-tools/` and are produced by the `designer` agent.
Scenario-to-mockup mapping: `docs/design/add-core-annotation-tools/index.md`.

Design approved: _pending_

## Technical decisions
- One module per tool implementing `Annotation`; shared property widgets generated from a property schema
- Golden-image tests per tool and per property
- Tool icons are symbolic SVGs from the design kit

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
