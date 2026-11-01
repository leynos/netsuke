//! Steps for bounded materialization in behavioural tests.
//!
//! The scenarios drive the public surface the way a consumer will: they build a
//! request from operand lengths, admit it, and read the typed refusal. No step
//! reaches into the kernel, a snapshot, or a private constructor, so each
//! scenario shows the outcome a template helper would actually observe.
//!
//! The refusal steps match on the public variants rather than comparing
//! `Display` text. That is deliberate: the English rendering is for developers
//! and logs (D-03), so a scenario that asserted wording would pin a string the
//! consuming modules must not depend on.

use anyhow::{Context as _, Result, ensure};
use netsuke::stdlib::{
    AdmittedCount, CardinalityError, CardinalityRequest, OUTPUT_TUPLE_CEILING,
    PERMUTATION_TUPLE_CEILING,
};
use rstest_bdd_macros::{given, then, when};

use crate::bdd::fixtures::TestWorld;

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

/// Set the operand lengths of a product request and its single repetition.
#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[given("a product request over operand lengths 316 and 317")]
pub(crate) fn request_over_316_and_317(world: &TestWorld) -> Result<()> {
    world.resource_lengths.set(vec![316, 317]);
    world.resource_repeat.set(1);
    Ok(())
}

/// Set the operand lengths of a product request whose base already overflows.
#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[given("a product request over operand lengths 4294967296 and 4294967296")]
pub(crate) fn request_over_two_to_the_thirty_two(world: &TestWorld) -> Result<()> {
    world
        .resource_lengths
        .set(vec![4_294_967_296, 4_294_967_296]);
    world.resource_repeat.set(1);
    Ok(())
}

/// Set a permutation request that arranges every item.
#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[given("a permutations request choosing 8 of 8 items")]
pub(crate) fn request_eight_of_eight(world: &TestWorld) -> Result<()> {
    world.resource_permutation.set((8, 8));
    Ok(())
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

/// Admit whichever combinatorial request the scenario set up.
#[when("the request is admitted")]
pub(crate) fn admit_the_request(world: &TestWorld) -> Result<()> {
    let outcome = if let Some((length, r)) = world.resource_permutation.get() {
        CardinalityRequest::Permutations { length, r }.admit()
    } else {
        let lengths = world
            .resource_lengths
            .get()
            .context("the scenario should have given operand lengths or a permutation")?;
        let repeat = world.resource_repeat.get().unwrap_or(1);
        CardinalityRequest::Product {
            lengths: &lengths,
            repeat,
        }
        .admit()
    };
    world
        .resource_admission
        .set(outcome.map(AdmittedCount::get));
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

/// Assert the refusal named both the cardinality and its ceiling.
#[then("admission is refused with cardinality 100172 and ceiling 100000")]
pub(crate) fn refused_with_100172(world: &TestWorld) -> Result<()> {
    assert_exceeded(world, 100_172, OUTPUT_TUPLE_CEILING)
}

/// Assert the refusal was overflow rather than a count.
#[then("admission is refused as overflow against ceiling 100000")]
pub(crate) fn refused_as_overflow(world: &TestWorld) -> Result<()> {
    match admission(world)? {
        Err(CardinalityError::Overflow { ceiling }) => ensure!(
            ceiling == OUTPUT_TUPLE_CEILING,
            "the refusal must name the output-tuple ceiling, not {ceiling}",
        ),
        other => anyhow::bail!("the request must be refused as overflow, not {other:?}"),
    }
    Ok(())
}

/// Assert the permutation refusal used the lower ceiling.
#[then("admission is refused with cardinality 40320 and ceiling 10000")]
pub(crate) fn refused_with_40320(world: &TestWorld) -> Result<()> {
    assert_exceeded(world, 40_320, PERMUTATION_TUPLE_CEILING)
}

// ---------------------------------------------------------------------------
// Helpers shared by the steps above
// ---------------------------------------------------------------------------

/// Read the outcome stored by the admission step.
fn admission(world: &TestWorld) -> Result<Result<usize, CardinalityError>> {
    world
        .resource_admission
        .get()
        .context("the scenario should have admitted a request")
}

/// Assert an exceeded refusal with the given cardinality and ceiling.
fn assert_exceeded(world: &TestWorld, cardinality: u64, ceiling: u64) -> Result<()> {
    match admission(world)? {
        Err(CardinalityError::Exceeded {
            cardinality: reported,
            ceiling: reported_ceiling,
        }) => {
            ensure!(
                reported == cardinality,
                "the refusal must name cardinality {cardinality}, not {reported}",
            );
            ensure!(
                reported_ceiling == ceiling,
                "the refusal must name ceiling {ceiling}, not {reported_ceiling}",
            );
        }
        other => {
            anyhow::bail!("the request must be refused with an exceeded verdict, not {other:?}")
        }
    }
    Ok(())
}
