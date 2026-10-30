# Design: Bootstrap app shell and Debian packaging

## Mockups (design gate)

Mockups live in `docs/design/bootstrap-app-shell/` (HTML in `src/`, PNG light/dark at 1× and 2× in `png/`).
Scenario-to-mockup mapping and redlines: [docs/design/bootstrap-app-shell/index.md](../../../docs/design/bootstrap-app-shell/index.md).

Screens (light, 1×; every screen also has `-dark@1x/2x`):

- [01-first-launch-welcome](../../../docs/design/bootstrap-app-shell/png/01-first-launch-welcome-light@1x.png)
- [02-main-menu](../../../docs/design/bootstrap-app-shell/png/02-main-menu-light@1x.png)
- [03-preferences-general](../../../docs/design/bootstrap-app-shell/png/03-preferences-general-light@1x.png)
- [04-preferences-format-dropdown](../../../docs/design/bootstrap-app-shell/png/04-preferences-format-dropdown-light@1x.png)
- [05-preferences-capture](../../../docs/design/bootstrap-app-shell/png/05-preferences-capture-light@1x.png)
- [06-about-dialog](../../../docs/design/bootstrap-app-shell/png/06-about-dialog-light@1x.png)
- [07-keyboard-shortcuts](../../../docs/design/bootstrap-app-shell/png/07-keyboard-shortcuts-light@1x.png)
- [08-editor-open-file](../../../docs/design/bootstrap-app-shell/png/08-editor-open-file-light@1x.png)
- [09-gnome-app-grid](../../../docs/design/bootstrap-app-shell/png/09-gnome-app-grid-light@1x.png)
- [10-gnome-search](../../../docs/design/bootstrap-app-shell/png/10-gnome-search-light@1x.png)
- [11-cli-errors](../../../docs/design/bootstrap-app-shell/png/11-cli-errors-light@1x.png)

Design approved: 2026-10-30

## Technical decisions
- Meson drives Cargo (GNOME template); Blueprint for UI files
- Single instance via GApplication D-Bus uniqueness
- Crates vendored to `debian/vendor` with `cargo vendor`; `debian/rules` uses `dh --buildsystem=meson`
- App ID `io.github.trosinde.Annota`

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
