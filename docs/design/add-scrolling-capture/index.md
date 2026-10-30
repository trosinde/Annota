# add-scrolling-capture — mockups

Change: `openspec/changes/add-scrolling-capture/`. Overlays 1920×1080; editor 1280×800. The browser window in the
scenes is a generic, invented one showing kit sample `f-field-notes` (1600 × 2600) at four scroll positions
(`src/assets/desk-s*.png`).

| # | Screen | File (light@1x) |
|---|---|---|
| 01 | Ready – region chosen, waiting for scroll | [png/01-scroll-start-light@1x.png](png/01-scroll-start-light@1x.png) |
| 02 | Capturing – live stitched preview, sticky header detected | [png/02-scroll-capturing-sticky-light@1x.png](png/02-scroll-capturing-sticky-light@1x.png) |
| 03 | Auto-scroll reached end of page | [png/03-auto-scroll-end-light@1x.png](png/03-auto-scroll-end-light@1x.png) |
| 04 | 20 000 px limit reached | [png/04-limit-reached-light@1x.png](png/04-limit-reached-light@1x.png) |
| 05 | Auto-scroll not allowed (no RemoteDesktop permission) | [png/05-auto-scroll-unavailable-light@1x.png](png/05-auto-scroll-unavailable-light@1x.png) |
| 06 | Lost overlap (scrolled too fast) | [png/06-stitch-lost-overlap-light@1x.png](png/06-stitch-lost-overlap-light@1x.png) |
| 07 | Result in editor (tall image, zoomed out) | [png/07-result-in-editor-light@1x.png](png/07-result-in-editor-light@1x.png) |
| 08 | Launcher – Scrolling mode | [png/08-launcher-scrolling-mode-light@1x.png](png/08-launcher-scrolling-mode-light@1x.png) |

## Scenario → mockup

| Requirement / Scenario | Mockups | Notes |
|---|---|---|
| Manual scrolling capture / Long page | 08 → (region overlay from add-screen-capture 02/03) → 01 → 02 → 07 | Enter finishes; no duplicate/missing rows *verified by test* (golden stitch). |
| Automatic scrolling / Auto stop | 08 (Auto-scroll on), 03 → 07; limit 04; permission 05 | Stops at end of content or 20 000 px. |
| Sticky header handling / Sticky header | 02, 03, 07 | Header band hatched violet in the overlay; result keeps it once at the top. |
| Stitch failure (implicit in matching) | 06 | Capture pauses until frames overlap again. |

## Redlines

**Capture frame** — 2 px Annota Coral border around the chosen region, outside dimmed `rgb(0 0 6 / 45%)`; Coral tab above the top-left: `capture-region` icon + "Scrolling region · 1400 px wide". The frame is never in the captured frames (ScreenCast crops inside it).

**Sticky band** — detected fixed header/footer: violet diagonal hatch (22 % / 8 %), 2 px `--annota-violet` edge toward the content, chip "pin icon · Sticky header · 68 px" right-aligned 24 px in.

**Side panel** — OSD panel docked right, 14 px from the screen edge, top 56 (below the top bar), bottom 24, width 242, radius 18, padding 14, gap 10. If the region overlaps it, it docks left.
- Header: state icon + title (Ready ⏸ / Capturing… Coral dot / Paused / Limit reached / End of page ✓) and dim direction "Vertical"/"Horizontal".
- Toggle: Manual | Auto-scroll (Auto-scroll dimmed 45 % and an info note when not permitted, 05).
- Meta line: "2,300 px captured" / "limit 20,000" (13 px, tabular), progress 6 px Coral.
- Preview: stitched image scaled to panel width (190 px), dashed seams where frames joined, Coral box = current viewport. When the image is taller than the panel it anchors to the bottom with "↑ N px above" (04).
- Notes (13 px, radius 9): info "Sticky header found — Kept once at the top of the result."; warning (amber 30 %) "Limit reached (20,000 px) — Capture stopped. Finish to open what was captured." / "Scrolled too fast — Scroll back up a little so the frames overlap."
- Buttons: "Cancel", "Finish" (suggested). Keys line: "Enter Finish · Esc Cancel".

**Ready cue (01, 05)** — OSD 220 px wide near the bottom of the region: `pan-down` 32 px, "Scroll the page to capture", dim "or switch to Auto-scroll".

**End (03)** — bottom hint "✓ End of page reached · opening editor…" for ~800 ms, then 07.

**Result (07)** — title `Scrolling capture YYYY-MM-DD HH-MM-SS`, subtitle `1600 × 2600 · Not saved`; zoom fits **width** to max 100 % but drawn here at 25 % so the whole page is visible; canvas top-aligned with 18 px margin and a vertical overlay scrollbar. Sidebar "Capture": Size, Source "Scrolling · N frames stitched", Sticky header "Kept once · 68 px". Toast "Stitched 4 frames · 1600 × 2600" + "Fit Width".

**Launcher (08)** — fourth mode "Scrolling" (icon `pan-down` as stand-in, caption "Not set" = no default shortcut). Rows: Direction toggle Vertical/Horizontal; Auto-scroll switch (subtitle "Scrolls for you where GNOME allows it"); suggested "Select Scrolling Region…" → standard region overlay.

## Open questions
- Icon: the kit has no scrolling-capture icon; `pan-down` is a stand-in. Request a `capture-scrolling` icon in the kit (I did not edit the kit).
- Horizontal scrolling: layout is the same with the preview strip rotated to a bottom panel; not drawn separately — confirm that is enough.
- Wayland RemoteDesktop permission prompt itself is GNOME UI (not drawn).
