# Add screen capture (region, window, screen)

**Roadmap phase:** MVP (milestone 0.1)

## Why
Capturing is the entry point of every Annota workflow. It must work on GNOME Wayland, where applications cannot read the screen directly, so everything goes through xdg-desktop-portal.

## What Changes
- Region, window and full-screen capture via the Screenshot portal
- Annota's own region-selection overlay with magnifier loupe, pixel dimensions and snap to window edges
- Delayed capture with countdown
- Global hotkey (GlobalShortcuts portal, fallback GNOME custom keybinding)
- Capture from clipboard and from file
- Capture is opened in the editor or copied, per preference

## Benchmark features covered
- Region / window / full screen capture
- Time delay
- Exact capture dimensions
- Custom keyboard shortcuts for capturing
- Include/exclude cursor
- New image from clipboard

## Out of scope
- Scrolling capture (Phase 3)
- Multiple areas, time-lapse capture, presets (Phase 2)
- Video (Phase 2)

## Capabilities

### New Capabilities
- `capture`

### Modified Capabilities
- `app-shell`: `--capture <mode>` now actually starts a capture instead of only acknowledging the request (completes the "Single application instance" requirement bootstrap-app-shell left as a placeholder)

## Impact
- New module `src/capture/`; dependency `ashpd`
