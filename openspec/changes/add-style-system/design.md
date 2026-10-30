# Design: Add quick styles and themes

## Mockups (design gate)

Mockups live in `docs/design/add-style-system/` (HTML in `src/`, PNG light/dark at 1× and 2× in `png/`).
Scenario-to-mockup mapping and redlines: [docs/design/add-style-system/index.md](../../../docs/design/add-style-system/index.md).

Screens (light, 1×; every screen also has `-dark@1x/2x`):

- [01-quick-style-grid](../../../docs/design/add-style-system/png/01-quick-style-grid-light@1x.png)
- [02-save-as-quick-style](../../../docs/design/add-style-system/png/02-save-as-quick-style-light@1x.png)
- [03-style-saved-context-menu](../../../docs/design/add-style-system/png/03-style-saved-context-menu-light@1x.png)
- [04-favourites-view](../../../docs/design/add-style-system/png/04-favourites-view-light@1x.png)
- [05-favourite-applied-callout](../../../docs/design/add-style-system/png/05-favourite-applied-callout-light@1x.png)
- [06-theme-editor](../../../docs/design/add-style-system/png/06-theme-editor-light@1x.png)
- [07-theme-applied](../../../docs/design/add-style-system/png/07-theme-applied-light@1x.png)
- [08-managed-theme-readonly](../../../docs/design/add-style-system/png/08-managed-theme-readonly-light@1x.png)
- [09-theme-import-export](../../../docs/design/add-style-system/png/09-theme-import-export-light@1x.png)
- [10-theme-import-error](../../../docs/design/add-style-system/png/10-theme-import-error-light@1x.png)

Design approved: 2026-10-30

## Technical decisions
- Styles stored in `~/.local/share/annota/styles/`; themes as JSON with schema version

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
