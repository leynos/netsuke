//! End-to-end tests for `make verus` against the pinned real verifier.
//!
//! The other prover-tool suites replace the verifier with a fake so they can
//! run anywhere. These cases do the opposite: they drive the real Make target,
//! the pinned `rust-prover-tools` CLI, and the pinned Verus binary, so the
//! default proof, an ordered batch, and failure propagation are exercised
//! through the whole stack. Three property cases keep the pinned CLI real but
//! swap in a fake verifier, which is the only way to observe the batching loop
//! without running Verus once per generated case.
//!
//! Verus is an optional maintainer tool and the repository deliberately has no
//! Verus CI job, so a host without the pinned install skips these cases. The
//! `test-verus-e2e` target sets `NETSUKE_VERUS_E2E_STRICT=1`, which turns that
//! skip into a failure for a maintainer who means to run them.

#![cfg(unix)]

#[path = "prover_tools_verus_e2e_tests/support.rs"]
mod support;

use anyhow::{Context, Result, bail, ensure};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use std::fs;
use std::path::Path;
use support::{
    BatchFixture, batch_flags, failure_cases, ordered_failure_cases, output_text,
    require_real_verifier, run_make_verus, verification_counts, verification_runs,
    write_distinct_proof,
};
use tempfile::tempdir;

/// The repository's canonical proof kernel, used as the default and as one
/// member of the ordered batch.
const CANONICAL_PROOF: &str = "verus/canonical_value/main.rs";

/// Convert a fixture error into the failure its proptest case reports.
///
/// The property cases below run the real pinned CLI, so a fixture failure is
/// about the host rather than about the generated case. Proptest shrinks a
/// failing case either way; travelling as the case's failure rather than as a
/// panic is what keeps the error's own text, with its context chain, in the
/// report.
fn case_failure(error: &anyhow::Error) -> TestCaseError {
    TestCaseError::fail(format!("{error:#}"))
}

/// Check that the default target verifies the repository's proof kernel.
#[test]
fn verus_verifies_the_default_proof_with_the_real_verifier() -> Result<()> {
    if !require_real_verifier()? {
        return Ok(());
    }

    let output = run_make_verus(None)?;

    ensure!(
        output.status.success(),
        "make verus should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        verification_runs(&output) == 1,
        "the default target should run exactly one proof"
    );
    let combined = output_text(&output);
    ensure!(
        combined.contains("0 errors"),
        "the real verifier should report no errors, got `{combined}`"
    );
    Ok(())
}

/// Check that repeated proof-file options run in the order given against the
/// real verifier.
///
/// The verifier prints one result line per proof and names no file, so two
/// proofs whose verified counts differ make the order observable. Running the
/// same pair in both orders and comparing the two count sequences is what
/// proves the CLI preserves the order it was given rather than, say, sorting or
/// running the same proof twice.
#[test]
fn verus_runs_an_ordered_batch_with_the_real_verifier() -> Result<()> {
    if !require_real_verifier()? {
        return Ok(());
    }

    let temp = tempdir().context("create distinct proof directory")?;
    let distinct = write_distinct_proof(temp.path())?;
    let canonical = Path::new(CANONICAL_PROOF);

    let forward = run_make_verus(Some(&batch_flags(canonical, &distinct)))?;
    ensure!(
        forward.status.success(),
        "make verus should succeed for a two-proof batch: {}",
        String::from_utf8_lossy(&forward.stderr)
    );
    let forward_counts = verification_counts(&forward);
    let [first, second] = forward_counts.as_slice() else {
        bail!("the batch should report one result per proof, got {forward_counts:?}");
    };
    ensure!(
        first != second,
        "the two proofs should be distinguishable by their verified counts, got {forward_counts:?}"
    );

    let reverse = run_make_verus(Some(&batch_flags(&distinct, canonical)))?;
    ensure!(
        reverse.status.success(),
        "make verus should succeed for the reversed batch: {}",
        String::from_utf8_lossy(&reverse.stderr)
    );
    let reverse_counts = verification_counts(&reverse);
    ensure!(
        reverse_counts == [*second, *first],
        "reversing the proof order should reverse the result order, \
         got {reverse_counts:?} for the reverse of {forward_counts:?}"
    );
    Ok(())
}

/// Check that a real verification failure reaches the caller's exit status.
#[test]
fn verus_propagates_a_real_verification_failure() -> Result<()> {
    if !require_real_verifier()? {
        return Ok(());
    }

    let temp = tempdir().context("create failing proof directory")?;
    let failing_proof = temp.path().join("failing.rs");
    fs::write(
        &failing_proof,
        concat!(
            "use vstd::prelude::*;\n",
            "\n",
            "verus! {\n",
            "\n",
            "proof fn deliberate_failure()\n",
            "    ensures 1 + 1 == 3,\n",
            "{\n",
            "}\n",
            "\n",
            "/// Provide the entry point Verus requires; it carries no proof of its own.\n",
            "fn main() {}\n",
            "\n",
            "} // verus!\n",
        ),
    )
    .context("write failing proof")?;

    let output = run_make_verus(Some(&format!("--proof-file {}", failing_proof.display())))?;

    ensure!(!output.status.success(), "make verus should fail");
    let combined = output_text(&output);
    ensure!(
        combined.contains("postcondition not satisfied"),
        "the real verifier should report the failed postcondition, got `{combined}`"
    );
    ensure!(
        combined.contains("prover-tools: target=verus failed exit="),
        "the Make target should report the propagated failure, got `{combined}`"
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(6))]

    /// Check that a successful batch runs every proof exactly once, in order.
    ///
    /// The pinned CLI is the real one; only the verifier is fake, so the
    /// ordering asserted here is the CLI's own batching behaviour.
    #[test]
    fn verus_runs_every_proof_of_a_successful_batch_in_order(count in 1usize..=4) {
        if !require_real_verifier().map_err(|error| case_failure(&error))? {
            return Ok(());
        }
        let fixture = BatchFixture::new(count).map_err(|error| case_failure(&error))?;

        let output = fixture.run(None).map_err(|error| case_failure(&error))?;

        prop_assert!(
            output.status.success(),
            "a successful batch should exit zero: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut expected = vec!["--version".to_owned()];
        expected.extend(fixture.proof_paths());
        prop_assert_eq!(fixture.invocations().map_err(|error| case_failure(&error))?, expected);
    }

    /// Check that a failing batch stops at the failing proof and reports its
    /// status.
    ///
    /// The executed prefix is the contract: proofs before the failure ran, the
    /// failing proof ran, and nothing after it did.
    #[test]
    fn verus_stops_at_the_first_failing_proof_and_propagates_its_status(
        (count, index, status) in failure_cases()
    ) {
        if !require_real_verifier().map_err(|error| case_failure(&error))? {
            return Ok(());
        }
        let fixture = BatchFixture::new(count).map_err(|error| case_failure(&error))?;

        let output = fixture.run(Some((index, status))).map_err(|error| case_failure(&error))?;

        prop_assert!(!output.status.success(), "a failing batch should not exit zero");
        let mut expected = vec!["--version".to_owned()];
        expected.extend(fixture.proof_paths().into_iter().take(index + 1));
        prop_assert_eq!(fixture.invocations().map_err(|error| case_failure(&error))?, expected);
        let stderr = String::from_utf8_lossy(&output.stderr);
        prop_assert!(
            stderr.contains(&format!("prover-tools: target=verus failed exit={status}")),
            "the target should report exit {status}, got `{stderr}`"
        );
    }

    /// Check that a generated batch order is run in that order and stops at the
    /// proof named to fail.
    ///
    /// The cases above stage their proofs in creation order, so a CLI that
    /// reordered its inputs would still agree with them. Here the staged order
    /// is a generated permutation, and the failing proof is named by its
    /// position in that order: the log must show the permutation itself, then
    /// the prefix through the failure and nothing after it.
    #[test]
    fn verus_follows_a_generated_batch_order_to_the_failing_proof(
        (order, index, status) in ordered_failure_cases()
    ) {
        if !require_real_verifier().map_err(|error| case_failure(&error))? {
            return Ok(());
        }
        let fixture = BatchFixture::in_order(&order).map_err(|error| case_failure(&error))?;

        let output = fixture.run(Some((index, status))).map_err(|error| case_failure(&error))?;

        prop_assert!(!output.status.success(), "a failing batch should not exit zero");
        let mut expected = vec!["--version".to_owned()];
        expected.extend(fixture.proof_paths().into_iter().take(index + 1));
        prop_assert_eq!(fixture.invocations().map_err(|error| case_failure(&error))?, expected);
        let stderr = String::from_utf8_lossy(&output.stderr);
        prop_assert!(
            stderr.contains(&format!("prover-tools: target=verus failed exit={status}")),
            "the target should report exit {status}, got `{stderr}`"
        );
    }
}
