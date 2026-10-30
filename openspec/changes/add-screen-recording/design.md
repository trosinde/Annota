# Design: Add screen recording

## Mockups (design gate)

Mockups live in `docs/design/add-screen-recording/` and are produced by the `designer` agent.
Scenario-to-mockup mapping: `docs/design/add-screen-recording/index.md`.

Design approved: _pending_

## Technical decisions
- GStreamer pipeline: pipewiresrc → videoconvert → x264enc/vp9enc; overlay compositing for cursor/drawings/webcam
- GIF via gifski
- Wayland: portal provides cursor metadata for highlight

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
