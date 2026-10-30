# Spec Delta

## Purpose
Finds and hides sensitive information in captures locally and guarantees that hidden data cannot be recovered from exports.

## ADDED Requirements

### Requirement: Automatic detection
Smart redaction SHALL detect e-mail addresses, phone numbers, IPv4/IPv6 addresses, URLs, credit-card numbers (Luhn-checked), IBANs, dates, API-key-like tokens and faces, locally.

#### Scenario: Email found
- **WHEN** a capture shows `jane@example.com`
- **THEN** a redaction object covers it, tagged as category email

### Requirement: Review and categories
Detected items SHALL be listed by category with toggles per category and per item; the user chooses blur, pixelate or solid for all.

#### Scenario: Exclude dates
- **WHEN** the user disables category dates
- **THEN** date redactions are removed from the canvas

### Requirement: Irreversible export
Exports SHALL always flatten redactions; `.annota` files SHALL store the redacted image only, discarding the original pixels under solid redactions when the user selects `Permanently redact`.

#### Scenario: Permanent
- **WHEN** the user chooses Permanently redact and saves
- **THEN** the saved original.png no longer contains the hidden pixels
