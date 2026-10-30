# Design: Add export and sharing

## Mockups (design gate)

Mockups live in `docs/design/add-export-share/` (HTML in `src/`, PNG light/dark at 1× and 2× in `png/`).
Scenario-to-mockup mapping and redlines: [docs/design/add-export-share/index.md](../../../docs/design/add-export-share/index.md).

Screens (light, 1×; every screen also has `-dark@1x/2x`):

- [01-share-menu](../../../docs/design/add-export-share/png/01-share-menu-light@1x.png)
- [02-export-dialog-png](../../../docs/design/add-export-share/png/02-export-dialog-png-light@1x.png)
- [03-export-dialog-jpg-size](../../../docs/design/add-export-share/png/03-export-dialog-jpg-size-light@1x.png)
- [04-export-dialog-webp-error](../../../docs/design/add-export-share/png/04-export-dialog-webp-error-light@1x.png)
- [05-export-dialog-pdf](../../../docs/design/add-export-share/png/05-export-dialog-pdf-light@1x.png)
- [06-quick-save-toast](../../../docs/design/add-export-share/png/06-quick-save-toast-light@1x.png)
- [07-clipboard-toast](../../../docs/design/add-export-share/png/07-clipboard-toast-light@1x.png)
- [08-drag-handle-hover](../../../docs/design/add-export-share/png/08-drag-handle-hover-light@1x.png)
- [09-drag-into-files](../../../docs/design/add-export-share/png/09-drag-into-files-light@1x.png)
- [10-pixelate-warning](../../../docs/design/add-export-share/png/10-pixelate-warning-light@1x.png)
- [11-batch-select-tray](../../../docs/design/add-export-share/png/11-batch-select-tray-light@1x.png)
- [12-batch-export-dialog](../../../docs/design/add-export-share/png/12-batch-export-dialog-light@1x.png)
- [13-batch-export-done](../../../docs/design/add-export-share/png/13-batch-export-done-light@1x.png)
- [14-email-handoff](../../../docs/design/add-export-share/png/14-email-handoff-light@1x.png)

Design approved: _pending_

## Technical decisions
- PDF via cairo PdfSurface, vector annotations kept vector
- Clipboard via GdkClipboard texture + PNG mime
- Email via `ashpd` Email portal

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
