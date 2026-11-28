//! Compile-time tests for Kani cfg wiring.
//!
//! These tests keep the `cfg(kani)` contract visible outside the Kani runner:
//! ordinary Cargo builds must know that `cfg(kani)` is intentional, while
//! misspelled cfg names should still be rejected when the same check-cfg policy
//! is applied.

use std::{io, path::PathBuf, process::Command};

use test_support::bounded_process::{RUN_DEADLINE, output_within};
use test_support::ui_fixture::{compile_fixture, stderr};

/// Verify repository policy files used by the `cfg(kani)` compile contract.
///
/// The fixture is compiled with the workspace `rustc` and then executed, so
/// its assertions run against the checked-in policy files. Trybuild previously
/// drove this case, but it discards ambient `RUSTFLAGS` and workspace
/// `build.rustflags`, so while Polonius was flag-gated it rebuilt the `netsuke`
/// dependency without the analysis and rejected the crate's `POLONIUS(...)`
/// sites (see docs/polonius.md). The pinned nightly now enables Polonius by
/// default, but the fixture needs no dependencies at all, so the direct
/// compile remains the simpler harness.
#[test]
fn compiled_fixture_validates_kani_cfg_policy_sources() -> io::Result<()> {
    let output_dir = tempfile::tempdir()?;
    let binary = output_dir.path().join("cfg-kani-policy-pass");
    let compile = compile_fixture(&fixture("cfg_kani_policy_pass.rs"), &binary, &KANI_CFG_ARGS)?;
    if !compile.status.success() {
        return Err(io::Error::other(format!(
            "policy fixture should compile:\n{}",
            stderr(&compile),
        )));
    }

    let run = output_within(&mut Command::new(&binary), RUN_DEADLINE)?;
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
    let output = rustc_with_kani_check_cfg("cfg_kani_compile_pass.rs")?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "cfg(kani) should be accepted:\n{}",
            stderr(&output),
        )));
    }
    Ok(())
}

/// Verify marker declarations fail to compile without their underscore prefix.
#[test]
fn marker_prefix_invariant_is_rejected_at_compile_time() -> io::Result<()> {
    let output = rustc_with_kani_check_cfg("cfg_kani_marker_prefix_compile_fail.rs")?;
    let stderr = stderr(&output);

    if output.status.success() {
        return Err(io::Error::other(
            "marker tokens without underscore prefixes should fail to compile",
        ));
    }
    if !stderr.contains("marker fallback in find_substitution only runs at underscore positions") {
        return Err(io::Error::other(format!(
            "marker-prefix diagnostic should explain the invariant:\n{stderr}",
        )));
    }
    Ok(())
}

/// Unknown cfg names fail under the same check-cfg declaration.
#[test]
fn unknown_cfg_is_rejected_by_compile_time_policy() -> io::Result<()> {
    let output = rustc_with_kani_check_cfg("unknown_cfg_compile_fail.rs")?;
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

/// Check-cfg policy every Kani fixture is compiled under.
const KANI_CFG_ARGS: [&str; 3] = ["--cfg=kani", "--check-cfg=cfg(kani)", "-Dunexpected-cfgs"];

/// Compile one fixture under the Kani check-cfg policy.
fn rustc_with_kani_check_cfg(source: &str) -> io::Result<std::process::Output> {
    let output_dir = tempfile::tempdir()?;
    compile_fixture(
        &fixture(source),
        &output_dir.path().join("ui-test-bin"),
        &KANI_CFG_ARGS,
    )
}

/// Return the absolute path of one checked-in UI fixture.
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/ui")
        .join(name)
}
