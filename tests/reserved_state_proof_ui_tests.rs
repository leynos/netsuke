//! Compile-time tests for the reservation proof that guards the reset.
//!
//! The force-node reset must not run without the refusal's proof, and the proof
//! must not be mintable outside the refusal. Both are type-level claims, so
//! they are pinned by compiling fixtures with the workspace compiler and
//! requiring the expected rejection. The fixtures mirror the proof's shape
//! because the real items are crate-private and a fixture crate cannot name
//! them at all.

use std::{io, path::PathBuf};

use test_support::ui_fixture::{compile_fixture, stderr};

/// Verify the fixture harness itself is sound.
///
/// The control mirrors the same modules and compiles, so a failure in the
/// rejection fixtures cannot be mistaken for a setup or harness fault.
#[test]
fn the_fixture_harness_compiles_the_control() -> io::Result<()> {
    let output = compile_fixture(
        &fixture("reserved_state_proof_control_pass.rs"),
        &tempfile::tempdir()?.path().join("proof-control-ui"),
        &[],
    )?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "the control fixture should compile:\n{}",
            stderr(&output),
        )));
    }
    Ok(())
}

/// Verify the proof cannot be constructed outside the refusal.
#[test]
fn the_reservation_proof_cannot_be_minted_outside_the_refusal() -> io::Result<()> {
    let output = compile_fixture(
        &fixture("reserved_state_proof_construction_fail.rs"),
        &tempfile::tempdir()?.path().join("proof-construction-ui"),
        &[],
    )?;

    if output.status.success() {
        return Err(io::Error::other(
            "the reservation proof must not be constructible outside its module",
        ));
    }
    let stderr = stderr(&output);
    if !stderr.contains("E0451") {
        return Err(io::Error::other(format!(
            "the rejection must be the private-field error:\n{stderr}",
        )));
    }
    Ok(())
}

/// Verify the reset cannot be called without a proof.
#[test]
fn the_reset_requires_a_reservation_proof() -> io::Result<()> {
    let output = compile_fixture(
        &fixture("reserved_state_proof_missing_fail.rs"),
        &tempfile::tempdir()?.path().join("proof-missing-ui"),
        &[],
    )?;

    if output.status.success() {
        return Err(io::Error::other(
            "reset_force_node must not compile without a reservation proof",
        ));
    }
    let stderr = stderr(&output);
    if !stderr.contains("E0061") {
        return Err(io::Error::other(format!(
            "the rejection must be the missing proof argument:\n{stderr}",
        )));
    }
    Ok(())
}

/// Return the absolute path of one checked-in UI fixture.
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/ui")
        .join(name)
}
