# Design: Add screen capture (region, window, screen)

## Mockups (design gate)

Produced by the `designer` agent. Scenario-to-mockup mapping and redlines: [`docs/design/add-screen-capture/index.md`](../../../docs/design/add-screen-capture/index.md).
Source HTML: `docs/design/add-screen-capture/src/`; dark and 2× renders sit next to each light@1x file in `png/`.

- [01-capture-launcher](../../../docs/design/add-screen-capture/png/01-capture-launcher-light@1x.png)
- [02-region-drag](../../../docs/design/add-screen-capture/png/02-region-drag-light@1x.png)
- [03-region-adjust-fixed-size](../../../docs/design/add-screen-capture/png/03-region-adjust-fixed-size-light@1x.png)
- [04-region-snap-window-edge](../../../docs/design/add-screen-capture/png/04-region-snap-window-edge-light@1x.png)
- [05-window-wayland-portal-picker](../../../docs/design/add-screen-capture/png/05-window-wayland-portal-picker-light@1x.png)
- [06-window-x11-hover](../../../docs/design/add-screen-capture/png/06-window-x11-hover-light@1x.png)
- [07-delay-countdown](../../../docs/design/add-screen-capture/png/07-delay-countdown-light@1x.png)
- [08-permission-denied](../../../docs/design/add-screen-capture/png/08-permission-denied-light@1x.png)
- [09-prefs-shortcuts-fallback](../../../docs/design/add-screen-capture/png/09-prefs-shortcuts-fallback-light@1x.png)
- [10-prefs-shortcuts-created](../../../docs/design/add-screen-capture/png/10-prefs-shortcuts-created-light@1x.png)
- [11-shortcut-capture-dialog](../../../docs/design/add-screen-capture/png/11-shortcut-capture-dialog-light@1x.png)
- [12-region-result-in-editor](../../../docs/design/add-screen-capture/png/12-region-result-in-editor-light@1x.png)
- [13-paste-from-clipboard](../../../docs/design/add-screen-capture/png/13-paste-from-clipboard-light@1x.png)
- [14-drop-image-file](../../../docs/design/add-screen-capture/png/14-drop-image-file-light@1x.png)

Design approved: 2026-10-30

## Technical decisions
- `ashpd` for Screenshot portal (non-interactive full-screen grab, then own overlay)
- Overlay is a fullscreen undecorated GtkWindow per monitor showing the frozen image
- HiDPI: capture at device pixels, store scale factor in document metadata
- X11 window picking via overlay hit-test on window geometry from `_NET_CLIENT_LIST_STACKING`

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
