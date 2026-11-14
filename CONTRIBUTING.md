# Contributing

Thanks for your interest! This project has a single maintainer (@trosinde).
Contributions are welcome via pull request; nobody except the maintainer can push to or merge into `main`.

## How to contribute

1. Fork the repository and create a branch from `main`.
2. Make your change. Add or update tests for it.
3. Run the test suite locally:
   `meson setup _build -Dprofile=development && meson test -C _build --print-errorlogs`
   and the lints: `cargo fmt --all --check` and `cargo clippy --locked --all-targets -- -D warnings`
4. Open a pull request against `main`.

## What happens next

- CI runs automatically on your pull request. First-time contributors with new GitHub accounts
  may have to wait until the maintainer approves the workflow run.
- The pull request is reviewed **only after all required checks are green**.
  If a check fails, please fix it by pushing to the same branch.
- If `main` has moved on, your branch must be updated before merging
  ("Update branch" button or rebase). Keep "Allow edits by maintainers" enabled so the maintainer can do this for you.
- The maintainer decides whether and when to merge. Pull requests are squash-merged.

## Reporting security issues

Please do not open public issues for vulnerabilities. See [SECURITY.md](SECURITY.md).
