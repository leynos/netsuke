//! Strategies and helpers shared by the obligation suites.
//!
//! Each obligation in this plan states its own domain, but a few pieces are
//! the same wherever they appear: the path domain `OBL-PATHKEY` reasons
//! about, the one place a graph is built from a specification, and the one
//! place it is emitted. Keeping them here rather than in each suite is what
//! stops the two suites from drifting into *slightly* different domains, which
//! is a defect no gate would catch — both would pass, and neither would be
//! evidence about the other.

use crate::ir::BuildGraph;
use crate::ninja_gen::determinism::strategy::graph::GraphSpec;
use crate::ninja_gen::generate_bundle;
use camino::Utf8PathBuf;
use proptest::prelude::*;
use proptest::test_runner::{RngSeed, TestCaseError, TestRunner};

/// The seed every coverage tally runs under.
///
/// A coverage assertion is a claim about the generator, not about one draw
/// from it, so it must not be able to fail because an unlucky seed
/// under-sampled a class. Proptest's default seed is random per run, which
/// makes every tally a small lottery: a class emitted a fraction of a per cent
/// of the time is genuinely missing from some runs, and the test then reports a
/// generator defect that does not exist.
///
/// The number is arbitrary and carries no meaning beyond being stable; what
/// matters is that it is fixed, so the verdict and the reported counts are
/// identical on every run and can be compared between them. The law suites
/// keep Proptest's random seed, because a law *is* worth checking against
/// inputs no earlier run has tried.
pub(crate) const COVERAGE_SEED: u64 = 0x4e45_5453_4b45; // "NETSKE" in ASCII.

/// The shrinking budget every graph-sized suite runs under, in milliseconds.
///
/// `EP-M0` question 4 measured why a wall is needed at all: a `GraphSpec` has
/// roughly 1,500 shrink dimensions, so a counter-example of 50 actions and 100
/// edges never reaches a minimum. The shrink loop completed about 940
/// candidates per second, and 30 seconds therefore buys roughly 28,000
/// candidates — enough to simplify a case, nowhere near enough to minimize one.
/// Left unbounded, shrinking instead runs into `.config/nextest.toml`'s
/// 300-second kill, and a killed process persists no regression seed, so the
/// failure becomes unreproducible.
///
/// The stop is deliberate rather than a concession: a partially shrunk case is
/// readable because `EP-M3` bounds the printed `Debug`, and a stable seed
/// matters more than a minimal witness. Proptest's own default, `0`, is "no
/// limit", so this constant is the whole mechanism.
///
/// The suites that draw a graph or a manifest and emit it all use this. The IR
/// property suites do not: their cases are `Vec<Vec<usize>>` over at most
/// thirty targets, which has a few dozen shrink dimensions rather than
/// thousands, and they converge without a wall.
pub(crate) const HEAVY_SHRINK_TIME_MS: u32 = 30_000;

/// A `TestRunner` for a coverage tally: a fixed seed over `cases` samples.
pub(crate) fn coverage_runner(cases: u32) -> TestRunner {
    TestRunner::new(ProptestConfig {
        cases,
        rng_seed: RngSeed::Fixed(COVERAGE_SEED),
        ..ProptestConfig::default()
    })
}

/// Greatest number of paths one generated list carries.
///
/// `OBL-PATHKEY` states its domain as 0 to 8 paths. The bound is small on
/// purpose: keying is a sort and a join, so a longer list adds cost without
/// reaching a shape the short lists cannot, and an injectivity violation needs
/// two draws to collide rather than a long one.
pub(crate) const MAX_PATHS: usize = 8;

/// Path lists over the keyable domain: non-empty, and free of NUL.
///
/// Both restrictions are preconditions of the injectivity claim rather than
/// conveniences. Joining with NUL cannot tell a separator from a NUL inside an
/// element, so `["a", "b"]` and `["a\0b"]` share one key; and an empty path is
/// indistinguishable from its absence, so `[]` and `[""]` do too. The
/// directed witnesses in `path_key.rs` record why each half is load-bearing.
///
/// The strategy constructs valid values rather than filtering for them.
/// Rejection sampling would spend the case budget discarding its own output,
/// and — worse — would make shrinking incoherent, since a rejected shrunk
/// candidate is indistinguishable from an uninteresting one.
pub(crate) fn keyable_paths() -> impl Strategy<Value = Vec<Utf8PathBuf>> {
    prop::collection::vec(keyable_path(), 0..=MAX_PATHS)
}

/// One non-empty path built from NUL-free characters.
fn keyable_path() -> impl Strategy<Value = Utf8PathBuf> {
    prop::collection::vec(non_nul_char(), 1..=8)
        .prop_map(|characters| Utf8PathBuf::from(characters.into_iter().collect::<String>()))
}

/// Any character except NUL, drawn without rejection.
///
/// The replacement is a `map` rather than a `prop_filter`, which keeps the
/// draw total. `\u{1}` is consequently over-represented; it is only ever a
/// path character, so the bias costs nothing.
fn non_nul_char() -> impl Strategy<Value = char> {
    any::<char>().prop_map(|character| {
        if character == '\0' {
            '\u{1}'
        } else {
            character
        }
    })
}

/// Build one materialization, reporting a generator defect as a test failure.
///
/// Returned rather than asserted so that a colliding pool reaches Proptest as
/// a shrinkable failure carrying the specification's shape, instead of as a
/// panic shrinking cannot reduce. The pool's slices are disjoint by
/// construction, so this is unreachable in a correct generator — which is
/// exactly why it must be reported rather than swallowed.
///
/// # Errors
///
/// Returns a test failure when the specification's pool slices collide.
pub(crate) fn materialize(spec: &GraphSpec, order: &[usize]) -> Result<BuildGraph, TestCaseError> {
    spec.materialize(order).map_err(|error| {
        TestCaseError::fail(format!("the pool's slices must not collide: {error}"))
    })
}

/// Emit one graph through the bundle entry point, as a shrinkable failure.
///
/// Every suite that reads emitted text needs the same two decisions, and
/// stating them once is what stops a suite from quietly adopting a different
/// sink or a different error. [`generate_bundle`] rather than
/// [`crate::ninja_gen::generate`] because the generated domains reach the
/// class intersection on purpose, so `graph_requires_dyndep` holds and the
/// simple path would reject exactly the interesting cases; and the failure is
/// returned rather than unwrapped so a generator defect arrives as a failure
/// carrying the case, instead of as a panic shrinking cannot reduce.
///
/// # Errors
///
/// Returns a test failure when the emitter refuses the graph.
pub(crate) fn emit(graph: &BuildGraph) -> Result<String, TestCaseError> {
    generate_bundle(graph)
        .map(|bundle| bundle.build_file().to_owned())
        .map_err(|error| {
            TestCaseError::fail(format!("a well-formed generated graph must emit: {error}"))
        })
}
