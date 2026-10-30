# Design: Bootstrap app shell and Debian packaging

## Mockups (design gate)

Mockups live in `docs/design/bootstrap-app-shell/` and are produced by the `designer` agent.
Scenario-to-mockup mapping: `docs/design/bootstrap-app-shell/index.md`.

Design approved: _pending_

## Technical decisions
- Meson drives Cargo (GNOME template); Blueprint for UI files
- Single instance via GApplication D-Bus uniqueness
- Crates vendored to `debian/vendor` with `cargo vendor`; `debian/rules` uses `dh --buildsystem=meson`
- App ID `io.github.trosinde.Annota`

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
