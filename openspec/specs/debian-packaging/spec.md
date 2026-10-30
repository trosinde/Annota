# debian-packaging Specification

## Purpose
Defines how Annota is built, packaged and installed as a native Debian/Ubuntu package.

## Requirements

### Requirement: Installable package
The project SHALL build a binary package `annota` with `dpkg-buildpackage -b` on Ubuntu 24.04 without network access.

#### Scenario: Offline build
- **WHEN** the package is built in a clean chroot without network
- **THEN** the build succeeds using vendored crates
- **AND** lintian reports no errors

### Requirement: Desktop integration
The package SHALL install a valid desktop file, AppStream metainfo, scalable and symbolic app icons, the GSettings schema and translations.

#### Scenario: App grid
- **WHEN** the package is installed on Ubuntu 24.04
- **THEN** Annota appears in the GNOME app grid with its icon
- **AND** `appstreamcli validate` and `desktop-file-validate` pass

### Requirement: Runtime dependencies
The package SHALL depend on xdg-desktop-portal and xdg-desktop-portal-gnome and recommend tesseract-ocr.

#### Scenario: Dependencies resolved
- **WHEN** the user installs the .deb with apt
- **THEN** all required portal packages are installed

### Requirement: Continuous integration
Every pull request SHALL run format, lint, tests, `openspec validate --all --strict` and a .deb build; the .deb SHALL be published as a CI artifact.

#### Scenario: Failing lint blocks
- **WHEN** a pull request introduces a clippy warning
- **THEN** the CI run fails
