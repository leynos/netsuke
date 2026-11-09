//! Confirm the Clippy configuration actually rejects unsanctioned sites.
//!
//! The workspace-wide `clippy.toml` forbids `std::io::IsTerminal::is_terminal`
//! so a presentation decision cannot be re-derived from ambient state, and the
//! entrypoint carries the one sanctioned exception. Reading the configuration
//! proves only that it is written down. The tests here compile real fixtures
//! with `clippy-driver` to prove the denial is live: an unsanctioned call is
//! rejected, and the same call under an explicit `#[expect]` is accepted, which
//! is the shape of the entrypoint's exception.
//!
//! `clippy-driver` is used directly rather than through `cargo clippy` because
//! the fixtures need no dependencies and must not start a nested Cargo build.
//! It discovers `clippy.toml` by walking up from the working directory, so each
//! fixture is compiled with its working directory set to the repository root.

use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use test_support::bounded_process::{COMPILE_DEADLINE, output_within};

/// The lint the fixtures are expected to trip.
///
/// Clippy prints this name unqualified in its `allow(...)` hint and hyphenated
/// in its `-D` note, so the underscore form is the one both spellings contain.
const DISALLOWED_LINT: &str = "disallowed_methods";

/// Reject a terminal probe that carries no contract.
///
/// This is the half that a configuration typo would silently break: if the
/// `disallowed-methods` entry were renamed, misspelled, or dropped, this test
/// fails even though the file still parses.
#[test]
fn clippy_rejects_an_unsanctioned_terminal_probe() -> io::Result<()> {
    let output = compile_with_clippy("tests/ui/clippy_terminal_probe_uncontracted.rs")?;
    let stderr = stderr(&output);
    if output.status.success() {
        return Err(io::Error::other(
            "an unsanctioned IsTerminal::is_terminal call must be rejected by Clippy",
        ));
    }
    for expected in [
        "use of a disallowed method",
        "std::io::IsTerminal::is_terminal",
        DISALLOWED_LINT,
    ] {
        if !stderr.contains(expected) {
            return Err(io::Error::other(format!(
                "the Clippy rejection should mention {expected:?}:\n{stderr}",
            )));
        }
    }
    Ok(())
}

/// Accept a terminal probe that states a scoped, reasoned exception.
///
/// This is the control half. Without it, a Clippy invocation that failed for
/// any unrelated reason — a missing toolchain, a bad flag, a syntax error in
/// the fixture — would satisfy the rejection test above.
#[test]
fn clippy_accepts_a_contracted_terminal_probe() -> io::Result<()> {
    let output = compile_with_clippy("tests/ui/clippy_terminal_probe_contracted.rs")?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "a probe under a scoped, reasoned exception should be accepted:\n{}",
            stderr(&output),
        )));
    }
    Ok(())
}

/// Compile one fixture with `clippy-driver` from the repository root.
///
/// The working directory matters: `clippy-driver` finds `clippy.toml` by
/// walking up from it, so running from a temporary directory would lose the
/// policy and the rejection test would pass vacuously.
fn compile_with_clippy(source: &str) -> io::Result<Output> {
    let root = repository_root();
    let source_path = root.join(source);
    let output_dir = tempfile::tempdir()?;
    // nested-cargo-allow: timed the spawn is bounded by `output_within`'s deadline
    let mut compile = Command::new(clippy_driver());
    compile
        .current_dir(&root)
        .arg("--edition=2024")
        .arg("--crate-type=bin")
        .arg("-D")
        .arg("warnings")
        .arg(source_path)
        .arg("-o")
        .arg(output_dir.path().join("clippy-ui-bin"));
    output_within(&mut compile, COMPILE_DEADLINE)
}

/// Return the workspace root the fixtures are resolved against.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Locate the Clippy driver matching the workspace toolchain.
///
/// `clippy-driver` sits beside `rustc` in the active toolchain, so a shim of the
/// same name is used when Rustup provides one. The shim honours
/// `rust-toolchain.toml`, which keeps these fixtures on the pinned nightly
/// rather than whatever toolchain happens to be first on `PATH`.
#[expect(
    clippy::disallowed_methods,
    reason = "locating a toolchain binary through PATH; there is no seam to inject and no process state to isolate"
)]
fn clippy_driver() -> PathBuf {
    std::env::var_os("CLIPPY_DRIVER")
        .map_or_else(|| Path::new("clippy-driver").to_path_buf(), PathBuf::from)
}

/// Decode a compile's standard error as text.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
