## MODIFIED Requirements

### Requirement: Runtime dependencies
The package SHALL depend on xdg-desktop-portal, xdg-desktop-portal-gnome, gstreamer1.0-pipewire and gstreamer1.0-plugins-base, and SHALL recommend tesseract-ocr. The two GStreamer packages provide the PipeWire source and the video conversion that window capture needs (`capture` "Window capture").

#### Scenario: Dependencies resolved
- **WHEN** the user installs the .deb with apt
- **THEN** all required portal packages are installed

#### Scenario: Window capture works on a fresh install
- **WHEN** the .deb is installed with apt on a fresh Ubuntu 24.04 desktop and the user captures a window
- **THEN** GStreamer's `pipewiresrc` and `videoconvert` elements are available and the capture delivers an image
