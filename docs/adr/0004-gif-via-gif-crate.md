# ADR 0004: GIF export via GStreamer and the `gif` crate, not gifski

Status: accepted by the maintainer 2026-11-22 (OpenSpec change `add-recording-extras`, design
question Q1)

## Context

CLAUDE.md §2 fixed "GIF via `gifski`" for video. gifski is licensed AGPL-3.0-or-later. Linked
into Annota it would put a stronger copyleft on the MIT-licensed binary, which CLAUDE.md §10
requires the maintainer to decide. On 2026-11-22 the maintainer decided against gifski. GIF
export has to use GStreamer elements already in the dependency set or an MIT/Apache-licensed
crate that builds with the project MSRV.

Measured on 2026-11-22:
- `gifenc` (gst-plugins-rs) is not packaged in any Ubuntu release or in Debian. Neither
  packages.ubuntu.com nor packages.debian.org lists `gstreamer1.0-plugins-rs` or a
  `gst-plugins-rs` source package.
- `avenc_gif` is missing from `gstreamer1.0-libav` 1.28.2 on Ubuntu 26.04. `gst-inspect-1.0`
  finds only `avdec_gif`. It would also add FFmpeg (universe) as a dependency.
- `gif` 0.14.2 (image-rs) is licensed MIT OR Apache-2.0 and declares `rust-version` 1.62. Its
  dependencies are `color_quant` 1.1.0 (MIT) and `weezl` 0.1.12 (MIT OR Apache-2.0). The MSRV-aware
  resolver resolved it for `rust-version = 1.91`, and a small encoder built and ran with Rust 1.93.

## Decision

GStreamer decodes the recording, applies the edit list and scales it:
`matroskademux` → decoder → `videorate` → `videoscale` → `videoconvert` → `appsink` (RGBA).
The `gif` crate then encodes the frames on a worker thread. Each frame gets its own palette
(NeuQuant through `color_quant`), and identical consecutive frames are merged by extending the
previous frame's delay. When Loop forever is on, the file carries an infinite-loop extension.

The §2 Video row becomes: "GStreamer (`gstreamer-rs`), output MP4 (H.264) / WebM (VP9); GIF via
GStreamer decoding + the `gif` crate (ADR 0004). GStreamer also takes the single frame of a
window still (ADR 0002)."

## Alternatives considered

- **gifski.** It has the best palette quality, but it is AGPL-3.0-or-later. Rejected by the
  maintainer.
- **`gifenc` from gst-plugins-rs.** It is not packaged for the target distributions, and
  vendoring a GStreamer plugin is out of scope for a native .deb.
- **`avenc_gif` from gst-libav.** It is absent from the distribution build, and it would pull in
  FFmpeg.
- **Dropping GIF.** Rejected: GIF export is a benchmark feature, and an MIT/Apache path exists.

## Consequences

- New crates: `gif`, `color_quant` and `weezl`, all MIT or MIT/Apache, listed in
  `debian/copyright` by `build-aux/update-copyright.py`. No new `.deb` dependency.
- Per-frame NeuQuant gives lower quality and larger files than gifski on gradients. That is
  acceptable for screen content, and the speed setting is tuned in the change's task 5.5.
- The project MSRV build (Rust 1.91 after `fix/zip-security-update`, before that 1.85) is
  verified when the crate is added (task 2.4).
