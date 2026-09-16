//! The `dev-fast` capability gate must stop a target before Cargo runs.
//!
//! Each case drives a sandbox holding a working, recording Cargo and varies
//! only the linker the gate inspects, so a failure here means Cargo was
//! reached. That matters because both build targets declare the check as a
//! prerequisite: a gate that reports failure but lets the recipe continue
//! would still fail `make`, just later and with a confusing linker error
//! instead of an actionable one.

use anyhow::{Result, ensure};
use rstest::rstest;
use test_support::dev_fast::{
    MakeInvocation, RecordingCargo, Sandbox, combined, pinned_mold_version, pinned_toolchain,
};

/// A missing `mold` must stop the target before Cargo is invoked.
///
/// Asserting on the recorded invocations proves that directly, where relying on
/// Cargo's absence from the sandbox would pass even if the recipe did invoke it.
#[rstest]
#[case("dev-build")]
#[case("dev-test")]
fn a_failed_gate_invokes_cargo_not_at_all(#[case] target: &str) -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    // A usable cargo is present and recording; only mold is missing.
    let cargo = RecordingCargo::install(&sandbox)?;

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
    Ok(())
}

/// A drifting `mold` fails the gate, so it must stop the build targets exactly
/// as a missing one does.
///
/// Worth asserting separately from the absent case: a drift leaves a perfectly
/// usable linker on `PATH`, so nothing but the gate's own verdict prevents the
/// recipe from proceeding.
#[rstest]
#[case("dev-build")]
#[case("dev-test")]
fn a_drifting_mold_invokes_cargo_not_at_all(#[case] target: &str) -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), "99.0.0")?;
    let cargo = RecordingCargo::install(&sandbox)?;

    let invocation = MakeInvocation::new(target).variable("CARGO", cargo.executable());
    let output = sandbox.run_make(&invocation)?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "`{target}` should fail on a drifting mold, got `{text}`"
    );
    // Pin the failure to its cause. Asserting only the exit status would let
    // this pass on an unrelated failure — a missing `rustup`, or a broken
    // recipe — and so would stop testing the drift gate at all.
    ensure!(
        text.contains("does not match the pin") && text.contains(&pinned_mold_version()?),
        "`{target}` should name the version mismatch and the pin, got `{text}`"
    );
    let recorded = cargo.invocations()?;
    ensure!(
        recorded.is_empty(),
        "`{target}` should not reach Cargo, recorded {} invocation(s)",
        recorded.len()
    );
    Ok(())
}
