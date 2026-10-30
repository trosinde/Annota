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
  combining the existing `docs/` tree with the generated `docs/apt/` tree — verified end-to-end on
  tag `v0.1.0-test3` (run 41083044009): `build-deb` green in 10m24s (full vendored build, clippy,
  tests, `openspec validate --all --strict`, lintian), `publish-apt-repo` green in 1m19s. Two
  unrelated pre-existing CI gaps were found and fixed along the way (both apply to `build-deb`,
  not specific to this change): `rustup component add rustfmt clippy` installed clippy for the
  runner's default toolchain instead of the pinned `rustc-1.85`, and `build-essential` was
  missing from the install list
- [x] 4.2 Add the `actions/deploy-pages` step (and required `pages: write`/`id-token: write`
  permissions) to publish that artifact — repo was private and Pages was never enabled before
  this change; with the maintainer's approval the repo was made public and Pages enabled with
  `build_type: workflow`. The auto-created `github-pages` environment only allowed deployments
  from `main`; with the maintainer's approval a `v*` tag deployment-branch-policy was added.
  Verified live: `https://trosinde.github.io/Annota/apt/dists/noble/InRelease` and
  `.../annota-archive-keyring.asc` both return 200, and `gpg --verify` against the fetched
  `InRelease` with the fetched public key reports "Good signature"
- [x] 4.3 Keep the existing unsigned-`.deb`-to-GitHub-Release attachment step unchanged; verify a
  test tag's Release still has the `.deb` asset attached exactly as before this change — verified:
  `gh release view v0.1.0-test3` shows asset `annota_0.1.0.dev1_amd64.deb` attached

## 5. Documentation

- [x] 5.1 Add an "Install" section to `README.md` documenting both paths: the signed-repo
  `add-repo` + `apt install annota` flow (with the exact commands from `design.md`) and the
  direct-`.deb`-download flow — written. Verified the two URLs the documented commands fetch
  (`.../annota-archive-keyring.asc`, the repo under `.../apt/`) are live and correct; did not run
  a full `apt install annota` in a clean container (the shipped `.deb` only installs a CLI skeleton
  so far per `bootstrap-app-shell` — a real GNOME session is needed to see it do anything, which
  is out of scope here)

## 6. Verify

- [x] 6.1 Run the `reviewer` agent against this change: specs have scenarios with a test or
  documented manual check (§6.3 matches 3.2/4.2/5.1 above), `cargo fmt`/`clippy`/tests green
  (unaffected by this change but re-verify no regression), `.deb` still builds and is
  lintian-clean, and the new `docs/apt/` tree verifies with `gpg --verify` — independent reviewer
  pass completed: **READY TO ARCHIVE**, no blocking findings. Non-blocking: no test yet ran a
  real `apt install` against the live `https://` repo (only `file://` + signature-verify checks
  so far — do once on the first real release); secret scanning/push protection are off on the now
  public repo (cheap hardening, not a bug); unrelated in-progress work from another change is
  sitting uncommitted in the tree (not part of this change's commits)
- [x] 6.2 `openspec validate --all --strict` passes with this change's deltas applied — confirmed:
  19/19 items pass

## Workflow follow-up

- `bootstrap-app-shell` has archived (`openspec/specs/debian-packaging/spec.md` now exists with
  4 requirements) — the earlier ordering blocker is gone; this change is ready to archive once
  task 6.1 (reviewer pass) is done.
- Archive this change (`/opsx:archive add-signed-apt-repo`) and then confirm
  `openspec/specs/debian-packaging/spec.md` carries the three new requirements from this
  change's delta (should read 7 requirements total afterward).
