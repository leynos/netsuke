//! Behavioural tests for the `dev-fast-check` capability gate.
//!
//! The gate's whole purpose is to turn a missing tool into an actionable
//! message instead of an opaque codegen-backend or linker failure, so these
//! tests assert on the diagnostics and exit status rather than on any build
//! artefact. Every case runs against fakes in a sandboxed `PATH`; none needs
//! mold, rustup, or a network.

#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Result, ensure};
use rstest::rstest;
use test_support::dev_fast::{Sandbox, combined, pinned_mold_version, pinned_toolchain};

/// A sandbox with everything present and matching the pins.
fn healthy_sandbox() -> Result<Sandbox> {
    let sandbox = Sandbox::new()?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    Ok(sandbox)
}

#[test]
fn reports_resolved_path_and_version_when_prerequisites_are_met() -> Result<()> {
    let sandbox = healthy_sandbox()?;
    let output = sandbox.make("dev-fast-check")?;
    let text = combined(&output);

    ensure!(output.status.success(), "check should pass, got `{text}`");
    ensure!(
        text.contains(&format!("mold {}", pinned_mold_version()?)),
        "should report the resolved version, got `{text}`"
    );
    ensure!(
        text.contains(&sandbox.prefix().join("bin/mold").to_string()),
        "should name the resolved path so an unexpected pick is visible, got `{text}`"
    );
    Ok(())
}

/// The regression this guards: the Makefile unconditionally exports
/// `$(HOME)/.local/bin` ahead of the caller's `PATH`, so an overridden
/// `DEV_FAST_PREFIX` used to be installed to but never selected.
#[test]
fn overridden_prefix_wins_over_a_mold_in_the_default_location() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    // A decoy in the location the Makefile's export would otherwise favour.
    sandbox.write_mold(&sandbox.home().join(".local/bin"), "0.0.0-decoy")?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;

    let output = sandbox.make("dev-fast-check")?;
    let text = combined(&output);

    ensure!(output.status.success(), "check should pass, got `{text}`");
    ensure!(
        text.contains(&sandbox.prefix().join("bin/mold").to_string()),
        "the overridden prefix should win PATH resolution, got `{text}`"
    );
    ensure!(
        !text.contains("0.0.0-decoy"),
        "the default-location mold should not be selected, got `{text}`"
    );
    Ok(())
}

#[test]
fn tolerates_a_version_drift_from_the_pin() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), "99.0.0")?;

    let output = sandbox.make("dev-fast-check")?;
    let text = combined(&output);

    ensure!(
        output.status.success(),
        "a newer mold is harmless and must not fail the check, got `{text}`"
    );
    ensure!(
        text.contains("run make install-dev-fast to match"),
        "drift should still be reported, got `{text}`"
    );
    Ok(())
}

/// Each unusable-tool case names the fault and the remedy, and exits non-zero
/// so `dev-build` and `dev-test` stop before Cargo runs.
/// `arrange` is carried as a function rather than dispatched on a name, so
/// adding a case cannot leave an unhandled arm behind.
#[derive(Copy, Clone)]
struct FailureCase {
    arrange: fn(&Sandbox) -> Result<()>,
    expected: &'static str,
}

fn without_mold(sandbox: &Sandbox) -> Result<()> {
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    Ok(())
}

/// A truncated download or an unresolved shared library: on `PATH`, but
/// incapable of reporting a version.
fn with_unrunnable_mold(sandbox: &Sandbox) -> Result<()> {
    sandbox.write_fake(&sandbox.prefix().join("bin"), "mold", "exit 1")?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    Ok(())
}

fn without_rustup(sandbox: &Sandbox) -> Result<()> {
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
    Ok(())
}

fn without_pinned_toolchain(sandbox: &Sandbox) -> Result<()> {
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
    sandbox.write_rustup("nightly-1970-01-01", true)?;
    Ok(())
}

fn without_cranelift_component(sandbox: &Sandbox) -> Result<()> {
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
    sandbox.write_rustup(&pinned_toolchain()?, false)?;
    Ok(())
}

#[rstest]
#[case::mold_absent(FailureCase { arrange: without_mold, expected: "mold not found on PATH" })]
#[case::mold_unrunnable(
    FailureCase { arrange: with_unrunnable_mold, expected: "cannot report its version" }
)]
#[case::rustup_absent(
    FailureCase { arrange: without_rustup, expected: "rustup not found on PATH" }
)]
#[case::toolchain_absent(
    FailureCase { arrange: without_pinned_toolchain, expected: "is not installed" }
)]
#[case::component_absent(
    FailureCase { arrange: without_cranelift_component, expected: "is not installed for" }
)]
fn unusable_prerequisites_fail_with_an_actionable_message(#[case] case: FailureCase) -> Result<()> {
    let sandbox = Sandbox::new()?;
    (case.arrange)(&sandbox)?;

    let output = sandbox.make("dev-fast-check")?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "case should fail the check, got `{text}`"
    );
    ensure!(
        text.contains(case.expected),
        "case should explain the fault (`{}`), got `{text}`",
        case.expected
    );
    ensure!(
        text.contains("make install-dev-fast") || text.contains("https://rustup.rs"),
        "case should point at a remedy, got `{text}`"
    );
    Ok(())
}

/// `dev-build` and `dev-test` depend on the check, so a missing prerequisite
/// must stop them before Cargo is invoked.
#[rstest]
#[case("dev-build")]
#[case("dev-test")]
fn build_targets_stop_when_the_check_fails(#[case] target: &str) -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    // No mold anywhere, and no cargo in the sandbox either: if the recipe ran,
    // the failure would name cargo rather than the capability check.
    let output = sandbox.make(target)?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "`{target}` should fail, got `{text}`"
    );
    ensure!(
        text.contains("mold not found on PATH"),
        "`{target}` should fail in the capability check, got `{text}`"
    );
    Ok(())
}
