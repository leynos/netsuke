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
//!
//! The stream scenario needs one thing the library does not export: a count of
//! how many items the collector pulled. The unit batteries have that fixture,
//! but it is compiled into the library's own test target and is not visible
//! here, so the step carries a local one. It is the only duplication, and it is
//! the observable the scenario exists to check.

use anyhow::{Context as _, Result, anyhow, ensure};
use netsuke::stdlib::{
    AdmittedCount, CardinalityError, CardinalityRequest, MATCH_COUNT_CEILING, NestingDepth,
    OUTPUT_TUPLE_CEILING, PERMUTATION_TUPLE_CEILING, StreamKind, collect_matches,
};
use rstest_bdd_macros::{given, then, when};

use crate::bdd::fixtures::TestWorld;

/// A stream that records how many times it was pulled.
#[derive(Debug)]
struct PullCounting<I> {
    /// The wrapped stream, in whatever shape the scenario needs.
    inner: I,
    /// Pulls observed so far.
    pulls: usize,
}

impl<I> PullCounting<I> {
    /// Wrap `inner`, starting its pull count at zero.
    const fn new(inner: I) -> Self {
        Self { inner, pulls: 0 }
    }

    /// Return how many times the stream has been pulled.
    const fn pulls(&self) -> usize {
        self.pulls
    }
}

impl<I: Iterator> Iterator for PullCounting<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.pulls = self.pulls.saturating_add(1);
        self.inner.next()
    }
}

/// Walk `levels` containers from the root, reporting the depth reached.
fn descend(levels: u32) -> Result<NestingDepth> {
    (0..levels)
        .try_fold(NestingDepth::root(), |depth, _| depth.descend())
        .map_err(|refusal| anyhow!("the first {levels} levels must be admitted: {refusal}"))
}

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

/// Walk to the deepest admitted level and record the depth reached.
#[given("a nesting depth at the ceiling")]
pub(crate) fn depth_at_the_ceiling(world: &TestWorld) -> Result<()> {
    let depth = descend(128)?;
    ensure!(
        depth.level() == 128,
        "the fixture must reach the ceiling, not stop short of it",
    );
    world.resource_depth.set(depth.level());
    Ok(())
}

/// Note that the stream under test has no end.
#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[given("an unbounded stream of matches")]
pub(crate) fn unbounded_match_stream(_world: &TestWorld) -> Result<()> {
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

/// Descend once past the depth the scenario reached.
#[when("the depth descends one more level")]
pub(crate) fn descend_past_the_ceiling(world: &TestWorld) -> Result<()> {
    let reached = world
        .resource_depth
        .get()
        .context("the scenario should have walked to the ceiling")?;
    let at_ceiling = descend(reached)?;
    let Err(refusal) = at_ceiling.descend() else {
        anyhow::bail!("one level past the ceiling must be refused");
    };
    world.resource_depth_refusal.set(refusal);
    Ok(())
}

/// Collect the unbounded stream and record how much of it was read.
#[when("the stream is collected under the match ceiling")]
pub(crate) fn collect_the_stream(world: &TestWorld) -> Result<()> {
    let mut stream = PullCounting::new(std::iter::repeat(1_u64));
    match collect_matches(&mut stream) {
        Ok(collected) => anyhow::bail!(
            "an unbounded stream must be refused, but {} items were collected",
            collected.len()
        ),
        Err(refusal) => world.resource_stream_refusal.set(refusal),
    }
    world.resource_pulled.set(stream.pulls());
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

/// Assert the refused descent named the level it attempted.
#[then("descent is refused with depth 129 and ceiling 128")]
pub(crate) fn refused_at_depth_129(world: &TestWorld) -> Result<()> {
    let refusal = world
        .resource_depth_refusal
        .get()
        .context("the scenario should have attempted one descent past the ceiling")?;
    ensure!(
        refusal.depth == 129,
        "the refusal must name depth 129, not {}",
        refusal.depth,
    );
    ensure!(
        refusal.ceiling == 128,
        "the refusal must name ceiling 128, not {}",
        refusal.ceiling,
    );
    Ok(())
}

/// Assert the collector stopped exactly one item past the ceiling.
#[then("collection is refused after pulling 100001 items")]
pub(crate) fn refused_after_100001_pulls(world: &TestWorld) -> Result<()> {
    let refusal = world
        .resource_stream_refusal
        .get()
        .context("the scenario should have refused the stream")?;
    ensure!(
        refusal.kind == StreamKind::MatchCount,
        "the refusal must name the match-count kind, not {:?}",
        refusal.kind,
    );
    ensure!(
        refusal.ceiling == MATCH_COUNT_CEILING,
        "the refusal must name the match ceiling, not {}",
        refusal.ceiling,
    );
    let pulled = world
        .resource_pulled
        .get()
        .context("the scenario should have counted its pulls")?;
    ensure!(
        pulled == 100_001,
        "the collector must stop after 100001 pulls, not {pulled}",
    );
    Ok(())
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
