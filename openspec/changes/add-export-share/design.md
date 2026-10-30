# Design: Add export and sharing

## Mockups (design gate)

Mockups live in `docs/design/add-export-share/` and are produced by the `designer` agent.
Scenario-to-mockup mapping: `docs/design/add-export-share/index.md`.

Design approved: _pending_

## Technical decisions
- PDF via cairo PdfSurface, vector annotations kept vector
- Clipboard via GdkClipboard texture + PNG mime
- Email via `ashpd` Email portal

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
