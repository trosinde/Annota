# Add smart redaction — mockups

Smart Redact lives in the Redact tool (`B`) sidebar. Sample: `e-relay-chat` (two e-mails, three phone numbers, three IP addresses, three dates, one booking reference as token); face demo uses a geometric avatar over the profile picture.

All screens: `src/NN-slug.html` → `png/NN-slug-{light,dark}@{1,2}x.png`. Render with `python3 tools/render-mockups.py docs/design/add-smart-redaction`.

## Scenario → mockup

| Scenario / state | Mockup | Notes |
|---|---|---|
| Automatic detection (running) | [01-detecting](png/01-detecting-light@1x.png) | Scan line + progress in sidebar; Cancel; note *Runs on this computer. Nothing is uploaded, and it works offline.* |
| Automatic detection › Email found | [02-review-detections](png/02-review-detections-light@1x.png) | Each hit becomes a redaction object outlined in Coral; the selected one shows its category tag (“E-mail”) and handles. Sidebar lists categories with counts and switches; expanded category lists items with checks. |
| Review and categories › Exclude dates | [03-exclude-dates](png/03-exclude-dates-light@1x.png) | Dates switch off → date redactions are removed from the canvas, the badge turns neutral. Also shows a single item unchecked (token): dashed outline, pixels visible, count drops. |
| Review › choose style for all | [02-review-detections](png/02-review-detections-light@1x.png), [04-style-pixelate-warning](png/04-style-pixelate-warning-light@1x.png), [05-style-blur](png/05-style-blur-light@1x.png) | Toggle group Solid / Pixelate / Blur applies to all smart redactions. Pixelate and Blur show the warning note. |
| Faces | [10-face-detected](png/10-face-detected-light@1x.png) | Face category with one item; blur clipped to the face shape. |
| Irreversible export › Permanent | [06-permanent-redaction-confirm](png/06-permanent-redaction-confirm-light@1x.png), [07-permanently-redacted](png/07-permanently-redacted-light@1x.png) | Permanently Redact… → alert (06) with option to switch blur/pixelate to Solid. After saving (07): subtitle “Saved · Permanently redacted”, confirmation card, toast. |
| Irreversible export (always flatten) | [08-export-warning](png/08-export-warning-light@1x.png) | Export dialog: *Flatten annotations* locked on (pin icon, disabled switch, explanation). Warning box when blur/pixelate is used, with *Use Solid for All*. |
| Nothing found (empty) | [09-nothing-found](png/09-nothing-found-light@1x.png) | Compact status page, hint to draw manually with `B`, *Check Again*. |

## Screens

- [01-detecting](png/01-detecting-light@1x.png) · [dark](png/01-detecting-dark@1x.png)
- [02-review-detections](png/02-review-detections-light@1x.png) · [dark](png/02-review-detections-dark@1x.png)
- [03-exclude-dates](png/03-exclude-dates-light@1x.png) · [dark](png/03-exclude-dates-dark@1x.png)
- [04-style-pixelate-warning](png/04-style-pixelate-warning-light@1x.png) · [dark](png/04-style-pixelate-warning-dark@1x.png)
- [05-style-blur](png/05-style-blur-light@1x.png) · [dark](png/05-style-blur-dark@1x.png)
- [06-permanent-redaction-confirm](png/06-permanent-redaction-confirm-light@1x.png) · [dark](png/06-permanent-redaction-confirm-dark@1x.png)
- [07-permanently-redacted](png/07-permanently-redacted-light@1x.png) · [dark](png/07-permanently-redacted-dark@1x.png)
- [08-export-warning](png/08-export-warning-light@1x.png) · [dark](png/08-export-warning-dark@1x.png)
- [09-nothing-found](png/09-nothing-found-light@1x.png) · [dark](png/09-nothing-found-dark@1x.png)
- [10-face-detected](png/10-face-detected-light@1x.png) · [dark](png/10-face-detected-dark@1x.png)

## Redlines
- **Entry point:** Redact tool (`redact` icon, shortcut `B`). Smart detection runs automatically when the tool is first opened on an image; menu item “Find Sensitive Data Again”.
- **Detection outline:** 2.5 px `--annota-coral` rectangle, 3 image px outside the redaction. Category tag only on the selected object: Coral pill 19 px high, white 13 px bold label (“E-mail”, “Phone”, “IP address”, “Web address”, “Card”, “IBAN”, “Date”, “Token”, “Face”).
- **Solid:** `--annota-ink` fill, radius 2. **Pixelate:** 10 image px cells. **Blur:** σ ≈ 7 px; faces use the face outline (circle) as clip.
- **Sidebar order:** header “Smart Redact” + “N items found · checked on this computer”; caption “Style for all” + full-width toggle group; optional warning note (amber 18 % bg, `warning` icon: “Blur and pixelate can sometimes be undone by others. Use Solid for passwords, keys and personal data.”); category boxed list; full-width *Permanently Redact…* (destructive text on neutral button).
- **Category rows (fixed order):** E-mail addresses, Phone numbers, IP addresses, Web addresses, Payment cards, IBANs, Dates, Keys and tokens, Faces. Row 44 px: chevron (when expanded), title (`--fs-small`), count badge (accent when on, neutral when off), switch (85 %). Categories with 0 hits: dimmed, “None”, disabled switch. Item rows 36 px, monospace text, check on the right.
- **Alert:** heading “Permanently Redact?”, body names the count, check “Switch blur and pixelate to Solid” (default on), buttons Cancel / *Redact and Save* (destructive).
- **Export dialog:** warning box above the fields: “N redactions use Pixelate/Blur” + *Use Solid for All*. Flatten row: subtitle “Always on: the image contains redactions”, `pin` icon, disabled switch.

Shared: editor shell = kit `template.html` (header 47 px, tool strip 52 px, properties 300 px), 6 px grid, Annota Coral `#ff5a4e` for annotation ink, Adwaita blue for UI accent. No accounts, cloud or network anywhere.
