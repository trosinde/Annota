# Design: Add core annotation tools

## Mockups (design gate)

Mockups live in `docs/design/add-core-annotation-tools/` (HTML in `src/`, PNG light/dark at 1× and 2× in `png/`).
Scenario-to-mockup mapping and redlines: [docs/design/add-core-annotation-tools/index.md](../../../docs/design/add-core-annotation-tools/index.md).

Light 1× renders:

- [01-tool-switch-tooltip](../../../docs/design/add-core-annotation-tools/png/01-tool-switch-tooltip-light@1x.png)
- [02-arrow-curve-end-styles](../../../docs/design/add-core-annotation-tools/png/02-arrow-curve-end-styles-light@1x.png)
- [03-arrow-double-dashed](../../../docs/design/add-core-annotation-tools/png/03-arrow-double-dashed-light@1x.png)
- [04-line](../../../docs/design/add-core-annotation-tools/png/04-line-light@1x.png)
- [05-pen](../../../docs/design/add-core-annotation-tools/png/05-pen-light@1x.png)
- [06-shape-frame](../../../docs/design/add-core-annotation-tools/png/06-shape-frame-light@1x.png)
- [07-shape-polygon-drawing](../../../docs/design/add-core-annotation-tools/png/07-shape-polygon-drawing-light@1x.png)
- [08-text-edit-in-place](../../../docs/design/add-core-annotation-tools/png/08-text-edit-in-place-light@1x.png)
- [09-callout-two-tails](../../../docs/design/add-core-annotation-tools/png/09-callout-two-tails-light@1x.png)
- [10-callout-shapes](../../../docs/design/add-core-annotation-tools/png/10-callout-shapes-light@1x.png)
- [11-step-sequence-type](../../../docs/design/add-core-annotation-tools/png/11-step-sequence-type-light@1x.png)
- [12-step-edit-value-shapes](../../../docs/design/add-core-annotation-tools/png/12-step-edit-value-shapes-light@1x.png)
- [13-step-renumber-on-delete](../../../docs/design/add-core-annotation-tools/png/13-step-renumber-on-delete-light@1x.png)
- [14-highlighter](../../../docs/design/add-core-annotation-tools/png/14-highlighter-light@1x.png)
- [15-redact-blur-pixelate-solid](../../../docs/design/add-core-annotation-tools/png/15-redact-blur-pixelate-solid-light@1x.png)
- [16-colour-picker](../../../docs/design/add-core-annotation-tools/png/16-colour-picker-light@1x.png)
- [17-eyedropper-active](../../../docs/design/add-core-annotation-tools/png/17-eyedropper-active-light@1x.png)
- [18-shadow-editor](../../../docs/design/add-core-annotation-tools/png/18-shadow-editor-light@1x.png)
- [19-export-redaction-warning](../../../docs/design/add-core-annotation-tools/png/19-export-redaction-warning-light@1x.png)
- [20-narrow-text-properties](../../../docs/design/add-core-annotation-tools/png/20-narrow-text-properties-light@1x.png)

Design approved: 2026-10-30

## Technical decisions
- One module per tool implementing `Annotation`; shared property widgets generated from a property schema
- Golden-image tests per tool and per property
- Tool icons are symbolic SVGs from the design kit

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
