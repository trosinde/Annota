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

### Requirement: Signed APT repository
Release builds SHALL be published to a GPG-signed APT repository in addition to the existing
unsigned `.deb` GitHub Release asset, so installs and upgrades are verified by `apt` itself.

#### Scenario: Install from the signed repository
- **WHEN** a user has added the Annota APT repository and imported its public key
- **THEN** `apt install annota` and later `apt upgrade` succeed
- **AND** `apt` rejects the package if the repository's `Release`/`InRelease` signature does not
  verify against the imported key

#### Scenario: Direct .deb download still available
- **WHEN** a release is published
- **THEN** the unsigned `.deb` is still attached to the GitHub Release as before
- **AND** installing it with `dpkg -i`/`apt install ./annota_*.deb` keeps working unchanged

### Requirement: Published signing key
The project's APT repository signing public key SHALL be published in the repository and on the
repository site, with documented import instructions.

#### Scenario: User imports the key before adding the repository
- **WHEN** a user follows the README "Install" instructions
- **THEN** they fetch `packaging/annota-archive-keyring.asc` (or the equivalent published URL)
- **AND** import it before `apt update` succeeds against the Annota repository

### Requirement: Key rotation and revocation documented
The maintainer SHALL have a documented procedure to rotate or revoke the release-signing key
without breaking `apt upgrade` for users who already trust the current key.

#### Scenario: Maintainer rotates the signing key
- **WHEN** the maintainer follows the documented rotation procedure
- **THEN** a new key is published and future releases are signed with it
- **AND** users who have not yet imported the new key are told how to pick it up, instead of
  upgrades silently failing
