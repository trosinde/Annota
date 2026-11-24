<div align="center">

<img src="data/icons/hicolor/scalable/apps/io.github.trosinde.Annota.svg" width="96" alt="">

# Annota

**Capture it. Mark it up. Make it obvious.**

Annotated screenshots and screen recordings for GNOME – editable until you export.

<img src="docs/assets/hero.png" width="860" alt="The Annota editor with a settings page marked up: numbered steps 1 to 3, a callout saying Turn this on, an arrow and two pixelated private fields">

</div>

You explain software with screenshots – in a support ticket, a how-to, a bug report or a handout.
A plain screenshot makes the reader guess where to look. Annota lets you point at exactly what
matters, hide what is private, and change your mind until the moment you share it.
And when a click path is easier to show than to describe, Annota records it as a video.
It is a native GNOME app for Ubuntu and Debian, and it never sends anything anywhere.

> **Status: pre-alpha.** Version 0.2.0 is out – [install it from the APT repository](#get-started).
> New in 0.2.0: **[screen recording](docs/how-to/record-the-screen.md)** of a region, a window
> or the whole screen, with audio, saved as MP4 or WebM. Capture, the editor, the annotation
> tools, styles and export work since 0.1.0. The capture library and text recognition are
> [planned](#roadmap).

## Why Annota

### Point at exactly what matters

Arrows, callouts, numbered steps, shapes, text and a highlighter – each one key away.

<img src="docs/assets/feature-1-markup.png" width="720" alt="A settings page with step badges 1 to 3, a red callout pointing at a switch and an arrow with the note Daily is safer">

### Change your mind later

Every annotation stays an object: move it, restyle it or delete it at any time. Save as `.annota`
and keep editing tomorrow.

<img src="docs/assets/feature-2-editable.png" width="720" alt="A selected callout with resize handles, and its properties in the sidebar: shape, fill, outline, font and padding">

### Share without oversharing

Blur, pixelate or black out names, e-mail addresses and keys before you share. Annota warns you
when blur or pixelate is not enough – and it never opens a network connection.

<img src="docs/assets/feature-3-redact.png" width="720" alt="A pixelated e-mail address is selected; the sidebar offers Blur, Pixelate and Solid and warns that blur and pixelate can sometimes be reversed">

### Look consistent in one click

Quick styles and themes give every arrow, callout and step the same colours and fonts.

<img src="docs/assets/feature-4-styles.png" width="720" alt="A selected arrow with eight quick styles for arrows in the sidebar, from the Annota theme">

### Show it as a video

Some steps are easier to show than to describe. Press the Record shortcut (`Super+Alt+R`) or
click **Record** in the start window, then record a region, a window or the whole screen – with
your microphone and the system audio if you like. Pause and mute while you record, press the
shortcut again to stop, and save the video as MP4 or WebM.
[How to record the screen](docs/how-to/record-the-screen.md)

<img src="docs/assets/recording-setup.png" width="720" alt="A region selected over a settings page in Video mode; next to it the recording setup: Region, Window or Screen, microphone and system audio with level meters, include pointer, a 3-second countdown and a red Start Recording button">

## Before and after

<img src="docs/assets/before-after.png" width="860" alt="Left: a plain settings page where nothing shows where to look. Right: the same page with three steps, a callout and the private data pixelated">

## Get started

Install Annota from its signed APT repository. On Ubuntu 24.04 or newer, or Debian 13:

```bash
curl -fsSL https://trosinde.github.io/Annota/annota-archive-keyring.asc \
  | sudo tee /usr/share/keyrings/annota-archive-keyring.asc
echo "deb [signed-by=/usr/share/keyrings/annota-archive-keyring.asc] \
  https://trosinde.github.io/Annota/apt noble main" \
  | sudo tee /etc/apt/sources.list.d/annota.list
sudo apt update && sudo apt install annota
```

`apt` checks every download against Annota's release key, and `sudo apt upgrade` brings you
new releases.

Prefer not to add a repository? Download the `.deb` from the
[latest release](https://github.com/trosinde/Annota/releases/latest) and install it with
`sudo apt install ./annota_*_amd64.deb`. You then install updates by hand. To build from
source instead, see [Install Annota](docs/how-to/install.md#build-from-source).

Annota opens with **Region**, **Window** and **Screen** for screenshots and **Record** for a
video. Pick a screenshot mode, and the capture opens in the editor. Next:
[your first annotated screenshot](docs/tutorials/first-steps.md) or
[your first screen recording](docs/how-to/record-the-screen.md).

<img src="docs/assets/welcome.png" width="720" alt="Annota's start window, Capture Anything, with four cards: Region, Window and Screen for screenshots and Record for a video, each with its keyboard shortcut">

Annota does not start, or still behaves like the old version after an update? See
[Troubleshooting](docs/how-to/troubleshooting.md).

## Learn more

- **Tutorial** – [Your first annotated screenshot](docs/tutorials/first-steps.md)
- **How-to guides** – [Install Annota](docs/how-to/install.md) · [Record the screen](docs/how-to/record-the-screen.md) · [Troubleshooting](docs/how-to/troubleshooting.md) · [Build the Debian package](docs/development/building.md)
- **Reference** – [Keyboard shortcuts](docs/reference/keyboard-shortcuts.md) · [Feature catalogue](docs/feature-catalog.md)
- **Explanation** – [Architecture](docs/explanation/architecture.md) · [Decision records](docs/adr/)
- **All documentation** – [docs/](docs/README.md)

## Roadmap

- **0.1.0 (released):** app shell and packaging · screen capture · editor and `.annota` format · core tools · quick styles and themes · export and share
- **0.2.0 (released):** screen recording – region, window or screen, microphone and system audio, pause and mute, MP4 and WebM
- **Next (MVP):** capture library
- **Phase 2:** recording extras (webcam, cursor effects, drawing on screen, trimming, GIF) · advanced tools · effects · capture presets · text recognition · smart redaction · library search, tags and collections · templates
- **Phase 3:** scrolling capture · step capture · smart move and simplify

Every change is specified in [`openspec/changes/`](openspec/changes) before it is built –
see [how Annota is built](docs/development/workflow.md).

## Project

[Contributing and workflow](docs/development/workflow.md) · Privacy: no accounts, no cloud, no
telemetry · License: MIT for code, CC-BY-SA-4.0 for artwork and docs.

Annota is an independent project and is not affiliated with any commercial screenshot tool.
