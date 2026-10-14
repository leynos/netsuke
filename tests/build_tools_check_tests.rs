//! Behavioural tests for the `check-build-tools` capability gate.
//!
//! The gate's whole purpose is to turn a missing tool into an actionable
//! message instead of an opaque codegen-backend or linker failure, so these
//! tests assert on the diagnostics and exit status rather than on any build
//! artefact. Every case runs against fakes in a sandboxed `PATH`; none needs
//! mold, rustup, or a network.

#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Result, ensure};
use rstest::rstest;
use test_support::build_tools::{
    MakeInvocation, PinOverrides, RecordingCargo, Sandbox, combined, pinned_mold_version,
    pinned_toolchain,
};

/// A sandbox with everything present and matching the pins.
fn healthy_sandbox() -> Result<Sandbox> {
    let sandbox = Sandbox::new()?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
    sandbox.write_rustup(&pinned_toolchain()?)?;
    Ok(sandbox)
}

#[test]
fn reports_resolved_path_and_version_when_prerequisites_are_met() -> Result<()> {
    let sandbox = healthy_sandbox()?;
    let output = sandbox.make("check-build-tools")?;
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
/// The documented promise is that macOS and Windows lose only the linker and
/// keep the rest. Faking `uname` is what makes that testable from Linux —
/// otherwise the branch is reachable only on hardware CI does not have, which
/// is exactly where an untested fallback rots.
#[test]
fn a_non_linux_host_skips_mold_and_still_passes() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?)?;
    // No mold anywhere: the point is that its absence stops mattering.
    sandbox.write_fake(&sandbox.bin(), "uname", "echo Darwin")?;

    let output = sandbox.script_with("check-build-tools.sh", PinOverrides::Omitted, &[])?;
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
        text.contains("toolchain") && text.contains(&pinned_toolchain()?),
        "the toolchain should still be required off Linux, got `{text}`"
    );
    Ok(())
}

/// The installer likewise skips the linker off Linux rather than failing, and
/// goes on to install the toolchain half.
#[test]
fn a_non_linux_host_skips_the_mold_install() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?)?;
    sandbox.write_fake(&sandbox.bin(), "uname", "echo Darwin")?;
    // A URL that would fail loudly if the download were ever attempted.
    let output = sandbox.script_with(
        "install-build-tools.sh",
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
    // The command, not merely that rustup was reached: the toolchain half
    // exists to put the pinned nightly on the machine, and a run that called
    // rustup for anything else would satisfy a looser assertion while leaving
    // the host without the toolchain the standard needs.
    let rustup = sandbox.rustup_invocations()?;
    let expected = format!("toolchain install {}", pinned_toolchain()?);
    ensure!(
        rustup.iter().any(|call| call.starts_with(&expected)),
        "the toolchain half should still run `{expected}`, recorded `{rustup:?}`"
    );
    Ok(())
}

/// The regression this guards: the Makefile unconditionally exports
/// `$(HOME)/.local/bin` ahead of the caller's `PATH`, so an overridden
/// `BUILD_TOOLS_PREFIX` used to be installed to but never selected.
#[test]
fn overridden_prefix_wins_over_a_mold_in_the_default_location() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?)?;
    // A decoy in the location the Makefile's export would otherwise favour.
    sandbox.write_mold(&sandbox.home().join(".local/bin"), "0.0.0-decoy")?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;

    let output = sandbox.make("check-build-tools")?;
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
    sandbox.write_rustup(&pinned_toolchain()?)?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), "99.0.0")?;

    let output = sandbox.make("check-build-tools")?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "a drifting mold should fail the check, got `{text}`"
    );
    ensure!(
        text.contains("run make install-build-tools to match"),
        "the remedy should be named, got `{text}`"
    );
    Ok(())
}

/// Each unusable-tool case names the fault and the remedy, and exits non-zero
/// so the build and gate targets stop before Cargo runs.
/// `arrange` is carried as a function rather than dispatched on a name, so
/// adding a case cannot leave an unhandled arm behind.
#[derive(Copy, Clone)]
struct FailureCase {
    arrange: fn(&Sandbox) -> Result<()>,
    expected: &'static str,
}

fn without_mold(sandbox: &Sandbox) -> Result<()> {
    sandbox.write_rustup(&pinned_toolchain()?)?;
    Ok(())
}

/// A truncated download or an unresolved shared library: on `PATH`, but
/// incapable of reporting a version.
fn with_unrunnable_mold(sandbox: &Sandbox) -> Result<()> {
    sandbox.write_fake(&sandbox.prefix().join("bin"), "mold", "exit 1")?;
    sandbox.write_rustup(&pinned_toolchain()?)?;
    Ok(())
}

fn without_rustup(sandbox: &Sandbox) -> Result<()> {
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
    Ok(())
}

fn without_pinned_toolchain(sandbox: &Sandbox) -> Result<()> {
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
    sandbox.write_rustup("nightly-1970-01-01")?;
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
fn unusable_prerequisites_fail_with_an_actionable_message(#[case] case: FailureCase) -> Result<()> {
    let sandbox = Sandbox::new()?;
    (case.arrange)(&sandbox)?;

    let output = sandbox.make("check-build-tools")?;
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
        text.contains("make install-build-tools") || text.contains("https://rustup.rs"),
        "case should point at a remedy, got `{text}`"
    );
    Ok(())
}

/// The build, gate, and benchmark targets depend on the check, so a missing
/// prerequisite must stop them before anything is invoked.
///
/// Two binaries are probed, not one, because they fail to different edits.
/// Cargo is what a benchmarking run is made of, so a `bench-build` that reached
/// it would report times for a tree compiled without the standard — the exact
/// comparison the benchmark exists to make. `whitaker` is a separate binary
/// again: a `lint-whitaker` that ran it anyway would publish Dylint's verdict
/// on a differently compiled tree. Neither is reachable in the sandbox except
/// through the recipe, so a marker from either is proof that it ran.
///
/// Both are installed rather than left absent. Inferring "it did not run" from
/// an empty `PATH` would also hold for a recipe that invoked the binary and
/// swallowed the failure, which is a different and much worse bug.
#[rstest]
#[case("build")]
#[case("test-nextest")]
#[case("lint-clippy")]
#[case("lint-whitaker")]
#[case("typecheck")]
#[case("bench-build")]
fn build_targets_stop_when_the_check_fails(#[case] target: &str) -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?)?;
    // No mold anywhere, which is the only fault; everything else the recipes
    // need is present and recording.
    let cargo = RecordingCargo::install(&sandbox)?;
    let whitaker_marker = sandbox.home().join("whitaker-ran");
    sandbox.write_fake(
        &sandbox.bin(),
        "whitaker",
        &format!(": > '{whitaker_marker}'"),
    )?;

    let invocation = MakeInvocation::new(target).variable("CARGO", cargo.executable());
    let output = sandbox.run_make(&invocation)?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "`{target}` should fail, got `{text}`"
    );
    ensure!(
        text.contains("mold not found on PATH"),
        "`{target}` should fail in the capability check, got `{text}`"
    );
    let recorded = cargo.invocations()?;
    ensure!(
        recorded.is_empty(),
        "`{target}` should not reach Cargo, recorded {} invocation(s)",
        recorded.len()
    );
    ensure!(
        !whitaker_marker.as_std_path().exists(),
        "`{target}` should not reach whitaker"
    );
    Ok(())
}
