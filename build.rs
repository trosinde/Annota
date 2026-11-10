//! Build-time identity of a development build, for the About dialog and `annota --version`.
//!
//! `0.1.0` stays the same across every build until the next release, so the version alone cannot
//! tell a freshly installed binary from a weeks-old one — which has already cost a debugging
//! session (a stale `/usr/local/bin/annota` looked like a capture bug). A development build
//! therefore carries its commit and build time.
//!
//! Release builds get nothing: `src/meson.build` passes `--release` only for meson's `release`
//! profile, so `PROFILE` tells the two apart, and the `.deb` keeps a version string that depends
//! on nothing but the source.

use std::path::Path;
use std::process::Command;

fn main() {
    if std::env::var("PROFILE").as_deref() == Ok("debug") {
        if let Some(id) = build_id() {
            println!("cargo:rustc-env=ANNOTA_BUILD_ID={id}");
        }
    }
    // Without this, cargo would keep the id of the commit this last ran on. `index` covers a
    // commit on the same branch, `HEAD` a branch switch. Neither exists in a release tarball.
    if let Ok(dir) = std::env::var("CARGO_MANIFEST_DIR") {
        for name in [".git/HEAD", ".git/index"] {
            let path = Path::new(&dir).join(name);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}

/// `<short commit>[-dirty], <build time>`, or just the time outside a git checkout.
fn build_id() -> Option<String> {
    let dir = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let time = build_time()?;
    let Some(commit) = git(&dir, &["rev-parse", "--short", "HEAD"]) else {
        return Some(time);
    };
    let dirty = git(&dir, &["status", "--porcelain"]).is_some_and(|out| !out.is_empty());
    let dirty = if dirty { "-dirty" } else { "" };
    Some(format!("{commit}{dirty}, {time}"))
}

fn build_time() -> Option<String> {
    run(Command::new("date").args(["-u", "+%Y-%m-%d %H:%M UTC"]))
}

/// A git command in the source tree, or `None` outside a checkout (a release tarball) or without
/// git. A build must never fail over this.
fn git(dir: &str, args: &[&str]) -> Option<String> {
    run(Command::new("git").arg("-C").arg(dir).args(args))
}

fn run(command: &mut Command) -> Option<String> {
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(text.trim().to_string())
}
