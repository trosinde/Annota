# Spec Delta

## ADDED Requirements

### Requirement: Recording runtime dependencies
The package SHALL depend on the GStreamer plugin packages that provide every element the recording and export pipelines use: `pipewiresrc`, `pulsesrc`, `level`, `audiomixer`, `videocrop`, `videorate`, `x264enc`, `opusenc`, `matroskamux`, `mp4mux`, `vp9enc`, `webmmux` and the AAC encoder chosen in design.md, on Ubuntu 24.04, Ubuntu 26.04 and Debian 13.

#### Scenario: Recording elements installed
- **WHEN** the .deb is installed with apt on a clean Ubuntu 24.04 desktop
- **THEN** `gst-inspect-1.0` finds every element listed in this requirement
