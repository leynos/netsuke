//! Differential property: a pure manifest means the same thing in both modes.
//!
//! The claim is narrow on purpose. It covers manifests built only from
//! constructs the test surface already admits — variables, macros, `foreach`
//! over literal lists, `when` comparisons, Boolean interpolation, the lexical
//! path filters, the collection filters, `timedelta`, and `now()` under an
//! injected clock — and says that the build surface and the test surface
//! produce the same manifest, the same graph, and the same Ninja text for
//! them. A manifest that reaches the host is covered by the corpus run, which
//! expectation is "equal *or* a typed refusal"; here every generated manifest
//! must load in both modes.
//!
//! Three comparison layers are recorded rather than one because each can drift
//! independently. The manifest catches a rendering difference; the [`GraphView`]
//! projection catches a difference in how the rendered manifest was interpreted;
//! and the Ninja text catches a difference in the final emitted build file.
//! Comparing only the manifest would call two loads equal whose recipes were
//! lowered differently.
//!
//! The generator is valid by construction rather than filtered after the fact:
//! a rejected case would silently shrink the domain and the property would
//! weaken without saying so. Two counters keep the coverage honest — every
//! construct must be generated at least ten times in a run, so a strategy that
//! stops producing, say, `timedelta` fails loudly instead of passing over a
//! smaller grammar.
//!
//! The generator's data model and strategy live in `spec.rs` and the
//! three-layer comparison in `compare.rs`; this module owns the property, the
//! run configuration, and the coverage floor.
//!
//! [`GraphView`]: netsuke::graph_view::GraphView

mod compare;
mod spec;

use anyhow::{Context as _, Result};
use proptest::test_runner::{Config as ProptestConfig, TestRunner};
use rstest::rstest;
use time::OffsetDateTime;
use time::macros::datetime;

use self::compare::compare_modes;
use self::spec::{Coverage, Tally, spec_strategy};
use super::fixtures::fixed_clock_config;

/// The instant both surfaces are configured with, so `now()` agrees.
const FIXED_INSTANT: OffsetDateTime = datetime!(2026-06-08 12:00:00 UTC);

/// How many generated manifests a run must contain.
const CASES: u32 = 128;

/// How many times each construct must be generated in a run.
///
/// Ten is low enough that a 128-case run clears it comfortably and high enough
/// that a construct which has dropped out of the strategy cannot pass by luck.
const MIN_CONSTRUCT_COVERAGE: usize = 10;

/// Every construct clears the per-run coverage floor.
fn assert_coverage(coverage: Coverage) {
    let required = [
        ("targets", coverage.targets),
        ("foreach targets", coverage.foreach_targets),
        ("when clauses", coverage.whens),
        ("Boolean interpolation", coverage.boolean),
        ("lexical path filters", coverage.path_filters),
        ("collection filters", coverage.collection_filters),
        ("timedelta", coverage.timedelta),
        ("now()", coverage.now),
        ("macro calls", coverage.macro_calls),
    ];
    for (name, count) in required {
        assert!(
            count >= MIN_CONSTRUCT_COVERAGE,
            "the strategy generated `{name}` {count} times, fewer than {MIN_CONSTRUCT_COVERAGE}; \
             the grammar is narrower than the property claims"
        );
    }
}

/// The build and test surfaces agree on every generated pure manifest.
#[rstest]
fn full_and_test_agree_on_generated_manifests() -> Result<()> {
    let config = fixed_clock_config()?.with_clock(netsuke::stdlib::fixed_clock(FIXED_INSTANT));
    let tally = Tally::default();
    let mut runner = TestRunner::new(ProptestConfig {
        cases: CASES,
        ..ProptestConfig::default()
    });
    runner
        .run(&spec_strategy(), |spec| {
            tally.record(spec.coverage());
            compare_modes(&spec, &config)
        })
        .context("the Full/Test differential property failed")?;
    assert_coverage(tally.get());
    Ok(())
}
