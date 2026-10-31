# Manual checks — add-core-annotation-tools

Every scenario in this change has automated coverage: unit tests for the pure tool/property
logic, golden-image tests for rendering (`tests/render/golden/*.png`), and 9 live-GTK tests
(`#[ignore]`d, run one per process under `GDK_BACKEND=x11 xvfb-run -a`) driving the real canvas
and sidebar. The one thing none of that can cover is the real xdg-desktop-portal `PickColor`
call the eyedropper makes — same reason as every portal interaction in this project: this is the
maintainer's real desktop, not a disposable sandbox, so a live call was never made from here.

**Live check pending:** see the "Eyedropper" row in
`openspec/changes/add-screen-capture/manual-checks.md` — run it in the same session as that
change's other portal checks (same underlying xdg-desktop-portal, same reason to batch them).
