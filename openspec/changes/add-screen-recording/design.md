# Design: Add screen recording

## Mockups (design gate)

Mockups live in `docs/design/add-screen-recording/` and were produced by the `designer` agent.
Scenario-to-mockup mapping and redlines: [`docs/design/add-screen-recording/index.md`](../../../docs/design/add-screen-recording/index.md).

- [01-recording-setup](../../../docs/design/add-screen-recording/png/01-recording-setup-light@1x.png)
- [02-webcam-overlay-options](../../../docs/design/add-screen-recording/png/02-webcam-overlay-options-light@1x.png)
- [03-countdown](../../../docs/design/add-screen-recording/png/03-countdown-light@1x.png)
- [04-recording-control](../../../docs/design/add-screen-recording/png/04-recording-control-light@1x.png)
- [05-paused-mic-muted](../../../docs/design/add-screen-recording/png/05-paused-mic-muted-light@1x.png)
- [06-draw-while-recording](../../../docs/design/add-screen-recording/png/06-draw-while-recording-light@1x.png)
- [07-swapped-full-webcam](../../../docs/design/add-screen-recording/png/07-swapped-full-webcam-light@1x.png)
- [08-video-editor-trim](../../../docs/design/add-screen-recording/png/08-video-editor-trim-light@1x.png)
- [09-video-editor-cut-range](../../../docs/design/add-screen-recording/png/09-video-editor-cut-range-light@1x.png)
- [10-export-gif](../../../docs/design/add-screen-recording/png/10-export-gif-light@1x.png)
- [11-export-progress](../../../docs/design/add-screen-recording/png/11-export-progress-light@1x.png)
- [12-export-mp4](../../../docs/design/add-screen-recording/png/12-export-mp4-light@1x.png)

Dark and 2× renders sit next to each file (`-dark@1x.png`, `@2x.png`).

Design approved: _pending_

## Technical decisions
- GStreamer pipeline: pipewiresrc → videoconvert → x264enc/vp9enc; overlay compositing for cursor/drawings/webcam
- GIF via gifski
- Wayland: portal provides cursor metadata for highlight

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
