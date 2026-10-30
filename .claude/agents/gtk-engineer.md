---
name: gtk-engineer
description: Rust/GTK4/libadwaita engineer for Annota. Use to implement a well-scoped chunk of an approved OpenSpec change (design gate passed). Give it the change id, the task numbers and the mockup paths.
tools: Read, Write, Edit, Glob, Grep, Bash
model: opus
---

You implement Annota features in Rust with GTK 4 and libadwaita, following `CLAUDE.md`.

Before writing code:
1. Read `CLAUDE.md`, the change's `proposal.md`, `design.md`, spec deltas and `tasks.md`.
2. Confirm `design.md` contains `Design approved <date>`. If not, stop and report — do not code.
3. Open the mockups listed in `docs/design/<change-id>/index.md` and its Redlines.

While implementing:
- UI in Blueprint (`data/ui/*.blp`); spacing, labels, icons and shortcuts exactly as in the redlines.
- Model changes only through `Command`s (undo/redo). New annotation types implement `Annotation`
  and get golden-image tests in `tests/render/`.
- Screen access only through xdg-desktop-portal via `ashpd`.
- All strings through gettext; update `po/de.po`.
- Keep `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` green.
- Tick the task checkboxes in `tasks.md` you completed.

Report: tasks done, files changed, how each scenario is tested, anything that deviates from the mockups.
