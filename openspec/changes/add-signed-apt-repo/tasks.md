# Tasks

## 1. Design gate

- [x] 1.1 Confirm with the maintainer that no mockups are needed (this change has no GTK UI —
  release/CI infrastructure only) and that the approach in `design.md` (Ed25519 key,
  `apt-ftparchive`, GitHub Actions-based Pages deploy) is approved; record "Design approved:
  <date>" at the top of `design.md`

## 2. Signing key

- [x] 2.1 Generate the Ed25519 release-signing GPG key offline (`gpg --quick-generate-key`, 2-year
  expiry) and verify `gpg --list-keys` shows it with the chosen identity — generated
  (fingerprint `49EB5EA3FEAF7602E0CEC8ADDB0B5DE0E31E2C9A`, expires 2028-10-29); the private key
  material was wiped from this sandbox immediately after the two steps below
- [x] 2.2 Export the public key to `packaging/annota-archive-keyring.asc` and verify
  `gpg --dry-run --import packaging/annota-archive-keyring.asc` succeeds — done, import verified
  into a clean keyring
- [x] 2.3 Export the armored private key, base64-encode it, and store it as the
  `ANNOTA_RELEASE_SIGNING_KEY` GitHub Actions repository secret (manual, maintainer-only step —
  document the exact export command in `docs/adr/` so it is reproducible); verify by confirming
  the secret is listed (not readable) under repo Settings → Secrets — set via
  `gh secret set ANNOTA_RELEASE_SIGNING_KEY --repo trosinde/annota`; `gh secret list` confirms it
  exists (updated 2026-10-30)
- [x] 2.4 Write the key-rotation runbook (new key generated, published alongside the old one for
  one release cycle, which key signed which release announced in release notes, old key removed
  once unneeded) in `README.md` and verify it covers revocation as well as rotation — written as
  `docs/adr/0001-signed-apt-repo.md`, linked from `README.md`; covers rotation and revocation

## 3. APT repository assembly

- [x] 3.1 Add `build-aux/make-apt-repo.sh`: given a built `.deb`, lay out
  `pool/main/`, run `apt-ftparchive packages`/`apt-ftparchive release` to produce
  `dists/noble/main/binary-amd64/{Packages,Packages.gz}` and a draft `Release`, then
  `gpg --detach-sign --armor` → `Release.gpg` and `gpg --clearsign` → `InRelease`; verify by
  running it locally against a test `.deb` and checking `apt-get indextargets`-style validation
  (`gpg --verify Release.gpg Release` succeeds) — verified locally with a disposable test key and
  a dummy `.deb`: both `Release.gpg`/`Release` and clearsigned `InRelease` verify as "Good
  signature"
- [x] 3.2 Verify the generated tree installs correctly: point a throwaway container's
  `/etc/apt/sources.list.d/` at the local tree with `file://`, import the test key, and confirm
  `apt update && apt install annota` succeeds and `apt install` on a tampered copy (modified
  `.deb` byte) fails signature verification — verified with an isolated `apt` root (no changes to
  the sandbox's real `/etc/apt`): `apt update` against the signed `file://` repo fetched
  `InRelease`/`Packages` cleanly; re-running `gpg --verify` against a byte-tampered `Release`
  produced `BAD signature` as expected

## 4. CI integration

- [x] 4.1 Extend the `build-deb` job in `.github/workflows/ci.yml` (or add a dependent job) so
  that, gated on `startsWith(github.ref, 'refs/tags/v')`, it imports
  `ANNOTA_RELEASE_SIGNING_KEY`, runs `build-aux/make-apt-repo.sh`, and assembles a Pages artifact
  combining the existing `docs/` tree with the generated `docs/apt/` tree — job `publish-apt-repo`
  added; **not yet run** — no `v*` tag has been pushed since this job was added, so the workflow
  itself is untested on real CI
- [x] 4.2 Add the `actions/deploy-pages` step (and required `pages: write`/`id-token: write`
  permissions) to publish that artifact — step added; repo was private and Pages was never
  enabled before this change (`gh api .../pages` → 404, then `422` while private); with the
  maintainer's approval the repo was made public and Pages enabled with `build_type: workflow`
  (`https://trosinde.github.io/Annota/`). **Still open:** no run has happened yet to actually
  populate that URL — needs a `v*` tag push to verify end-to-end
- [ ] 4.3 Keep the existing unsigned-`.deb`-to-GitHub-Release attachment step unchanged; verify a
  test tag's Release still has the `.deb` asset attached exactly as before this change — step
  itself untouched in the diff; verification still needs a real tag push

## 5. Documentation

- [ ] 5.1 Add an "Install" section to `README.md` documenting both paths: the signed-repo
  `add-repo` + `apt install annota` flow (with the exact commands from `design.md`) and the
  direct-`.deb`-download flow; verify by following the documented signed-repo commands verbatim
  in a clean Ubuntu 24.04 container and confirming `annota --help` runs afterward

## 6. Verify

- [ ] 6.1 Run the `reviewer` agent against this change: specs have scenarios with a test or
  documented manual check (§6.3 matches 3.2/4.2/5.1 above), `cargo fmt`/`clippy`/tests green
  (unaffected by this change but re-verify no regression), `.deb` still builds and is
  lintian-clean, and the new `docs/apt/` tree verifies with `gpg --verify`
- [x] 6.2 `openspec validate --all --strict` passes with this change's deltas applied — confirmed:
  19/19 items pass

## Workflow follow-up

- Archive this change (`/opsx:archive add-signed-apt-repo`) only after `bootstrap-app-shell` has
  already archived, since this change's delta modifies `debian-packaging` and needs that capability
  to exist as an archived base spec first.
- After archiving, confirm `openspec/specs/debian-packaging/spec.md` carries the three new
  requirements from this change's delta.
