# Design: Add capture library (browse and manage captures)

## Mockups (design gate)

Mockups live in `docs/design/add-capture-library/` and are produced by the `designer` agent.
Scenario-to-mockup mapping: `docs/design/add-capture-library/index.md`.

Design approved: _pending_

## Technical decisions
- Files stay the source of truth; SQLite index (`rusqlite`) in `~/.cache/annota/library.db` is rebuildable
- Thumbnails cached per XDG thumbnail spec (normal/large) in `~/.cache/thumbnails`
- `GtkGridView`/`GtkColumnView` with lazy thumbnail loading; `notify` crate for folder watching
- Metadata: application/window title via portal response where available, else from X11 properties; URL only where the capture source provides it

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
