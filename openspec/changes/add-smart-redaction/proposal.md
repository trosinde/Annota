# Add smart redaction

**Roadmap phase:** Phase 2 (milestone 0.2–0.4)

## Why
Screenshots leak e-mail addresses, IPs and tokens. Automatic detection reduces the risk of sharing sensitive data.

## What Changes
- Detect e-mail, phone, IP, URL, IBAN/credit card patterns, dates, API-key-like strings and faces
- Review list with per-category toggles
- Safe export: redactions are always flattened; blur/pixelate warning

## Benchmark features covered
- Smart Redact
- Solid redaction
- Flatten before share

## Out of scope
- Cloud-based detection

## Capabilities

### New Capabilities
- `privacy-redaction`

### Modified Capabilities
- none

## Impact
- Depends on the MVP changes being archived first.
