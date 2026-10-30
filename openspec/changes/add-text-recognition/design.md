# Design: Add text recognition (grab and edit text)

## Mockups (design gate)

Mockups live in `docs/design/add-text-recognition/` and were produced by the `designer` agent.
Scenario-to-mockup mapping and redlines: [`docs/design/add-text-recognition/index.md`](../../../docs/design/add-text-recognition/index.md).

- [01-select-text-area](../../../docs/design/add-text-recognition/png/01-select-text-area-light@1x.png)
- [02-grab-text-dialog](../../../docs/design/add-text-recognition/png/02-grab-text-dialog-light@1x.png)
- [03-text-copied-toast](../../../docs/design/add-text-recognition/png/03-text-copied-toast-light@1x.png)
- [04-grab-whole-image-loading](../../../docs/design/add-text-recognition/png/04-grab-whole-image-loading-light@1x.png)
- [05-no-text-found](../../../docs/design/add-text-recognition/png/05-no-text-found-light@1x.png)
- [06-edit-text-before](../../../docs/design/add-text-recognition/png/06-edit-text-before-light@1x.png)
- [07-edit-text-after](../../../docs/design/add-text-recognition/png/07-edit-text-after-light@1x.png)
- [08-text-capture-select-region](../../../docs/design/add-text-recognition/png/08-text-capture-select-region-light@1x.png)
- [09-text-capture-notification](../../../docs/design/add-text-recognition/png/09-text-capture-notification-light@1x.png)
- [10-preferences-ocr-languages](../../../docs/design/add-text-recognition/png/10-preferences-ocr-languages-light@1x.png)
- [11-tesseract-not-installed](../../../docs/design/add-text-recognition/png/11-tesseract-not-installed-light@1x.png)
- [12-grab-text-narrow](../../../docs/design/add-text-recognition/png/12-grab-text-narrow-light@1x.png)

Dark and 2× renders sit next to each file (`-dark@1x.png`, `@2x.png`).

Design approved: _pending_

## Technical decisions
- `leptess` bindings or tesseract CLI; package recommends tesseract-ocr-eng/deu

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
