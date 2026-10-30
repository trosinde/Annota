# Signed APT repository for releases

**Roadmap phase:** Packaging/release infrastructure (not a benchmarked product feature; depends on the
MVP packaging baseline from `bootstrap-app-shell`, not on any capture/editing phase).

## Why
`build-aux/make-deb.sh` currently runs `dpkg-buildpackage -us -uc -b`: the `.deb` CI produces and
attaches to GitHub Releases is completely unsigned. A user installing it today has no way to verify
the package actually came from this project and was not tampered with in transit or on GitHub's
release storage. Distributing it through a GPG-signed APT repository lets `apt` itself verify every
install and upgrade against the maintainer's public key, which is the standard trust mechanism Debian
and Ubuntu users already rely on for every other package on their system.

## Benchmark features covered
None — this is packaging/release infrastructure, not a product feature benchmarked against Snagit or
any other capture/annotation tool. `docs/feature-catalog.md` has no corresponding entry.

## What Changes
- Generate a new GPG release-signing key for Annota (offline-generated keypair; the private key is
  stored only as a GitHub Actions secret, never committed; the public key is published in the repo
  at `packaging/annota-archive-keyring.asc` and served from the APT repo site).
- Extend the release job in `.github/workflows/ci.yml` (currently `build-deb`, runs on push to `main`
  and on `v*` tags) so that on a `v*` tag it additionally:
  - assembles a minimal `amd64` APT repository layout (`Packages`, `Packages.gz`, `Release` signed
    with the release key, `InRelease`) from the built `.deb`, and
  - publishes that repository to GitHub Pages under `docs/apt/`, alongside the existing
    `docs/index.html` project page already served from GitHub Pages.
- Keep attaching the unsigned `.deb` to the GitHub Release itself, as today, for users who prefer a
  direct download without adding a repository — the APT repo is an additional, not a replacement,
  distribution path.
- Document in `README.md` how a user adds the repository and imports
  `packaging/annota-archive-keyring.asc`, then installs with `apt install annota`; and how the
  maintainer rotates or revokes the signing key.
- **BREAKING**: none — this only adds a new, opt-in install path and a release artifact; the existing
  unsigned `.deb` download keeps working exactly as before.

## Capabilities

### Modified Capabilities
- `debian-packaging`: adds a signing/distribution requirement (signed APT repository, published
  public key, documented install/rotation flow) on top of the existing "Installable package",
  "Continuous integration" requirements. **Note:** `debian-packaging` was introduced by
  `bootstrap-app-shell` (`openspec/changes/bootstrap-app-shell/specs/debian-packaging/spec.md`),
  which is not yet archived (13/14 tasks done) — there is no archived base spec at
  `openspec/specs/debian-packaging/spec.md` yet. This change's delta is written to apply cleanly once
  `bootstrap-app-shell` archives; `tasks.md` records that archival as a prerequisite for `apply`.

## Impact
- `.github/workflows/ci.yml`: new steps in (or a new job following) `build-deb`, gated on `v*` tags.
- New `packaging/` directory: public keyring file, APT repo assembly script.
- `README.md`: new "Install" section documenting both the signed-repo and direct-`.deb` paths.
- GitHub Actions secrets: one new secret holding the release-signing private key.
- GitHub Pages content under `docs/apt/` (new), served alongside the existing `docs/index.html`.
- No changes to the application binary, `debian/control`, or runtime dependencies.
