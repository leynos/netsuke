//! UI coverage for the clippy `disallowed_methods` environment policy.
//!
//! The AGENTS.md environment mandate bans in-process `std::env` mutation
//! (`set_var`, `remove_var`, `set_current_dir`); `clippy.toml` and
//! `test_support/clippy.toml` encode that ban as a `disallowed-methods` list.
//! This suite proves the policy actually fires on a deliberate violation and
//! stays quiet for the sanctioned `Command` builder surface, so a future edit
//! that lifts one side of the policy fails these UI checks rather than only
//! the lint target.
//!
//! The compile-fail source is generated inside a temporary crate with the
//! banned path spliced from string fragments, so no forbidden literal ever
//! appears in a committed `.rs` file and `scripts/check-env-mutation.sh`
//! remains green while the policy is exercised end to end.

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::tempdir;

/// The banned path is assembled from pieces so the repository source never
/// contains the literal contiguously (the grep gate would otherwise reject
/// its own fixture); `concat!` still yields the exact path at compile time.
fn banned_set_current_dir() -> &'static str {
    concat!("std::env", "::set_current_dir")
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[expect(
    clippy::disallowed_methods,
    reason = "locating Cargo through the environment; there is no seam to inject and no process state to isolate"
)]
fn cargo() -> PathBuf {
    std::env::var_os("CARGO").map_or_else(|| Path::new("cargo").to_path_buf(), PathBuf::from)
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Run `cargo clippy` on one fixture crate with the environment policy denied.
fn clippy_on(manifest: &Path) -> io::Result<Output> {
    // A fresh target directory per invocation, or Cargo would reuse a stale
    // fingerprint from the earlier splice failure and skip re-linting.
    let target_dir = tempdir()?;
    Command::new(cargo())
        .arg("clippy")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--")
        .arg("-D")
        .arg("clippy::disallowed_methods")
        .env("CARGO_TARGET_DIR", target_dir.path())
        .output()
}

/// A deliberate process-CWD mutation is rejected by the policy.
#[test]
fn set_current_dir_compile_fails_under_the_policy() -> io::Result<()> {
    let temporary_root = tempdir()?;
    let fixture_dir = temporary_root.path().join("clippy_env_policy_compile_fail");
    fs::create_dir_all(fixture_dir.join("src"))?;
    fs::write(
        fixture_dir.join("Cargo.toml"),
        "[package]\nname = \"clippy-env-policy-compile-fail\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n",
    )?;
    // Clippy discovers `disallowed-methods` from a `clippy.toml` in the crate
    // directory. Copy the repository's real policy so this fixture exercises
    // exactly the configuration that gates the workspace build.
    fs::copy(
        manifest_dir().join("clippy.toml"),
        fixture_dir.join("clippy.toml"),
    )?;
    // The banned call is assembled from `banned_set_current_dir()` so the
    // repository text never matches the gate while still exercising it.
    let source = format!(
        "use std::path::Path;\nfn main() {{\n    let _ = {}(\"/tmp\");\n    let _ = Path::new(\"/tmp\");\n}}\n",
        banned_set_current_dir()
    );
    fs::write(fixture_dir.join("src/main.rs"), source)?;

    let output = clippy_on(&fixture_dir.join("Cargo.toml"))?;
    if output.status.success() {
        return Err(io::Error::other(
            "a deliberate process-CWD mutation should fail clippy's disallowed_methods",
        ));
    }
    let rendered = stderr(&output);
    if !rendered.contains("disallowed_methods") {
        return Err(io::Error::other(format!(
            "the rejection should name the disallowed_methods lint:\n{rendered}"
        )));
    }
    Ok(())
}

/// The sanctioned `Command` builder surface stays available under the policy.
#[test]
fn command_builders_pass_under_the_policy() -> io::Result<()> {
    let manifest = manifest_dir().join("tests/ui/clippy_env_policy_pass/Cargo.toml");
    let output = clippy_on(&manifest)?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "the sanctioned Command builder fixture should compile clean:\n{}",
            stderr(&output),
        )));
    }
    Ok(())
}
