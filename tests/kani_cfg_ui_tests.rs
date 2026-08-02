//! Compile-time tests for Kani cfg wiring.
//!
//! These tests keep the `cfg(kani)` contract visible outside the Kani runner:
//! ordinary Cargo builds must know that `cfg(kani)` is intentional, while
//! misspelled cfg names should still be rejected when the same check-cfg policy
//! is applied.

#![expect(
    clippy::disallowed_methods,
    reason = "locating build artefacts Cargo reports through the environment; there is no seam to inject and no process state to isolate"
)]

use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// Verify repository policy files used by the `cfg(kani)` compile contract.
///
/// The fixture is compiled with the workspace `rustc` and then executed, so
/// its assertions run against the checked-in policy files. Trybuild
/// previously drove this case, but trybuild discards ambient `RUSTFLAGS`
/// and workspace `build.rustflags`, so it rebuilt the `netsuke` dependency
/// without `-Zpolonius=next` and rejected the crate's `POLONIUS(...)`
/// sites; the fixture needs no dependencies, so a direct compile preserves
/// the contract (see docs/polonius.md).
#[test]
fn compiled_fixture_validates_kani_cfg_policy_sources() -> io::Result<()> {
    let output_dir = tempfile::tempdir()?;
    let binary = output_dir.path().join("cfg-kani-policy-pass");
    let compile = compile_ui_fixture("tests/ui/cfg_kani_policy_pass.rs", &binary)?;
    if !compile.status.success() {
        return Err(io::Error::other(format!(
            "policy fixture should compile:\n{}",
            stderr(&compile),
        )));
    }

    let run = Command::new(&binary).output()?;
    if !run.status.success() {
        return Err(io::Error::other(format!(
            "policy fixture assertions should pass:\n{}",
            stderr(&run),
        )));
    }
    Ok(())
}

/// `cfg(kani)` compiles when the expected check-cfg declaration is active.
#[test]
fn cfg_kani_is_accepted_by_compile_time_policy() -> io::Result<()> {
    let output = rustc_with_kani_check_cfg("tests/ui/cfg_kani_compile_pass.rs")?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "cfg(kani) should be accepted:\n{}",
            stderr(&output),
        )));
    }
    Ok(())
}

/// Unknown cfg names fail under the same check-cfg declaration.
#[test]
fn unknown_cfg_is_rejected_by_compile_time_policy() -> io::Result<()> {
    let output = rustc_with_kani_check_cfg("tests/ui/unknown_cfg_compile_fail.rs")?;
    let stderr = stderr(&output);

    if output.status.success() {
        return Err(io::Error::other("unknown cfg should be rejected by rustc"));
    }
    if !stderr.contains("unexpected `cfg` condition name") {
        return Err(io::Error::other(format!(
            "stderr should explain the unexpected cfg:\n{stderr}",
        )));
    }
    if !stderr.contains("netsuke_unknown_cfg_for_ui_test") {
        return Err(io::Error::other(format!(
            "stderr should name the rejected cfg:\n{stderr}",
        )));
    }
    Ok(())
}

fn rustc_with_kani_check_cfg(source: &str) -> io::Result<Output> {
    let output_dir = tempfile::tempdir()?;
    compile_ui_fixture(source, &output_dir.path().join("ui-test-bin"))
}

fn compile_ui_fixture(source: &str, output_path: &Path) -> io::Result<Output> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_path = manifest_dir.join(source);

    Command::new(rustc())
        .arg("--edition=2024")
        .arg("--crate-type=bin")
        .arg("--check-cfg=cfg(kani)")
        .arg("-Dunexpected-cfgs")
        .arg(source_path)
        .arg("-o")
        .arg(output_path)
        .output()
}

#[expect(
    clippy::disallowed_methods,
    reason = "locating build artefacts Cargo reports through the environment; there is no seam to inject and no process state to isolate"
)]
fn rustc() -> PathBuf {
    std::env::var_os("RUSTC").map_or_else(|| Path::new("rustc").to_path_buf(), PathBuf::from)
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
