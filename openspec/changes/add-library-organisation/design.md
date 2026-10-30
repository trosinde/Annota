# Design: Add library organisation (search, tags, collections, filters)

## Mockups (design gate)

Mockups live in `docs/design/add-library-organisation/` and are produced by the `designer` agent.
Scenario-to-mockup mapping: `docs/design/add-library-organisation/index.md`.

Design approved: _pending_

## Technical decisions
- SQLite FTS5 for full-text search; OCR text indexed in background with low priority
- Collections stored as item membership in the item metadata plus an index table

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
