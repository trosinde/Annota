# Design: Add smart redaction

## Mockups (design gate)

Mockups live in `docs/design/add-smart-redaction/` and were produced by the `designer` agent.
Scenario-to-mockup mapping and redlines: [`docs/design/add-smart-redaction/index.md`](../../../docs/design/add-smart-redaction/index.md).

- [01-detecting](../../../docs/design/add-smart-redaction/png/01-detecting-light@1x.png)
- [02-review-detections](../../../docs/design/add-smart-redaction/png/02-review-detections-light@1x.png)
- [03-exclude-dates](../../../docs/design/add-smart-redaction/png/03-exclude-dates-light@1x.png)
- [04-style-pixelate-warning](../../../docs/design/add-smart-redaction/png/04-style-pixelate-warning-light@1x.png)
- [05-style-blur](../../../docs/design/add-smart-redaction/png/05-style-blur-light@1x.png)
- [06-permanent-redaction-confirm](../../../docs/design/add-smart-redaction/png/06-permanent-redaction-confirm-light@1x.png)
- [07-permanently-redacted](../../../docs/design/add-smart-redaction/png/07-permanently-redacted-light@1x.png)
- [08-export-warning](../../../docs/design/add-smart-redaction/png/08-export-warning-light@1x.png)
- [09-nothing-found](../../../docs/design/add-smart-redaction/png/09-nothing-found-light@1x.png)
- [10-face-detected](../../../docs/design/add-smart-redaction/png/10-face-detected-light@1x.png)

Dark and 2× renders sit next to each file (`-dark@1x.png`, `@2x.png`).

Design approved: _pending_

## Technical decisions
- Pattern detection on OCR output; faces with a bundled small ONNX/OpenCV model (license checked)

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
