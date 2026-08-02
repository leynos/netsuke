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
use test_support::dev_fast::{
    PinOverrides, Sandbox, combined, pinned_mold_version, pinned_toolchain,
};

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

/// On a host without `mold`, the check reports the platform-linker fallback and
/// still passes.
///
/// The documented promise is that macOS and Windows keep Cranelift and lose
/// only the linker. Faking `uname` is what makes that testable from Linux —
/// otherwise the branch is reachable only on hardware CI does not have, which
/// is exactly where an untested fallback rots.
#[test]
fn a_non_linux_host_skips_mold_and_still_passes() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    // No mold anywhere: the point is that its absence stops mattering.
    sandbox.write_fake(&sandbox.bin(), "uname", "echo Darwin")?;

    let output = sandbox.script_with("dev-fast-check.sh", PinOverrides::Omitted, &[])?;
    let text = combined(&output);

    ensure!(
        output.status.success(),
        "a non-Linux host should pass without mold, got `{text}`"
    );
    ensure!(
        text.contains("mold is Linux-only") && text.contains("Darwin"),
        "should name the fallback and the host, got `{text}`"
    );
    ensure!(
        text.contains("rustc-codegen-cranelift-preview"),
        "Cranelift should still be required off Linux, got `{text}`"
    );
    Ok(())
}

/// The installer likewise skips the linker off Linux rather than failing, and
/// goes on to install the toolchain half.
#[test]
fn a_non_linux_host_skips_the_mold_install() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    sandbox.write_fake(&sandbox.bin(), "uname", "echo Darwin")?;
    // A URL that would fail loudly if the download were ever attempted.
    let output = sandbox.script_with(
        "install-dev-fast.sh",
        PinOverrides::Omitted,
        &[("MOLD_RELEASE_BASE_URL", "file:///nonexistent".to_owned())],
    )?;
    let text = combined(&output);

    ensure!(
        output.status.success(),
        "the install should succeed off Linux, got `{text}`"
    );
    ensure!(
        text.contains("skipping on Darwin"),
        "should say why the linker was skipped, got `{text}`"
    );
    ensure!(
        !text.contains("downloading"),
        "no download should be attempted off Linux, got `{text}`"
    );
    let rustup = sandbox.rustup_invocations()?;
    ensure!(
        rustup.iter().any(|call| call.starts_with("component add")),
        "the toolchain half should still run, recorded `{rustup:?}`"
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

/// An advisory pin is not a pin: a `mold` that does not match it must fail the
/// gate, so the linker actually used cannot silently diverge from the one the
/// repository claims.
#[test]
fn rejects_a_version_drift_from_the_pin() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), "99.0.0")?;

    let output = sandbox.make("dev-fast-check")?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "a drifting mold should fail the check, got `{text}`"
    );
    ensure!(
        text.contains("run make install-dev-fast to match"),
        "the remedy should be named, got `{text}`"
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
