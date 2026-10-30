# Manual checks — bootstrap-app-shell

Scenarios that need a real GTK display (no headless test harness yet) or that are only
partly in scope for this change. Run on Ubuntu 24.04/26.04 with `meson devenv -C _build
./src/annota`, GSettings schema installed (`meson install` or `glib-compile-schemas` into
the dev profile's schema dir).

| Scenario | Status | How to check |
|---|---|---|
| Setting persists | Implemented | Preferences → General: change "Default format" to JPG, quit (`Ctrl+Q`), relaunch, open Preferences again — JPG is still selected. Same for the Capture page's radios, combo, delay and switch. |
| Open shortcuts | Implemented | Press `Ctrl+?` in the main window — the shortcuts window opens with 5 groups (General, Capture, Document, Tools, More Tools). Also reachable from the main menu → "Keyboard Shortcuts". |
| System switches to dark | Implemented (inherited) | Toggle GNOME Settings → Appearance → Dark while Annota is open — window and preferences dialog restyle without restart. No Annota-specific code is involved; `AdwApplication` does this for every libadwaita window. |
| No network at all / No outgoing connections | Implemented (architectural) | `grep -rn "reqwest\|hyper\|curl\|TcpStream\|UdpSocket" src/ Cargo.toml` returns nothing outside dependency metadata — the binary links no networking crate. |
| Second launch forwards request | Implemented | `annota --capture region` while Annota is running: the existing window raises and a toast reads "Capturing arrives in milestone 0.1"; `ps` shows only one `annota` process. Matches the scenario's current wording (forward + handle); starting an actual region capture is `add-screen-capture`'s own, separately-tracked scenario (see that change's tasks.md 4.5). |
| Open file acknowledged before the editor exists | Implemented | `annota --open shot.annota` raises the window and shows a toast saying opening files isn't available yet. Matches the scenario's current wording; actually opening an editor is `add-annotation-editor`'s own, separately-tracked scenario (see that change's tasks.md 4.4). |

Automated coverage: `src/cli.rs` unit tests cover mode parsing and the delay range (the
"Invalid mode" scenario). `meson test` additionally runs `validate-desktop`,
`validate-metainfo` and `validate-schema` against the shipped data files.

Verified 2026-10-30 on this repo's dev container (Ubuntu 26.04, Xvfb, cargo/rustc 1.93) —
every row above and the GSettings-persistence row was exercised by hand; see the reviewer
agent's pass on this change for the full gate (fmt/clippy/tests/openspec validate) and its
findings, since fixed.
