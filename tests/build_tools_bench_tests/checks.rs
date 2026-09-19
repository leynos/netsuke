//! The checks a recorded benchmark run is held to.
//!
//! Split from the crate root to keep both readable: the root holds the
//! observable-behaviour cases, and this module holds the reusable machinery
//! they share — what a recorded pass must look like, and what a run of them
//! must satisfy. The per-variant contract lives in [`variant`].
//!
//! Only one property stages a run of its own, through the same hermetic fixture
//! the crate root uses; the rest are checkable against synthetic input.

#[path = "checks/variant.rs"]
mod variant;

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use proptest::prelude::*;
use proptest::proptest;
use proptest::test_runner::{FileFailurePersistence, TestCaseError};
use test_support::build_tools::{
    BENCH_REPEATS, BENCH_SLUGS, BuildScenario, CargoInvocation, DEFAULT_SLUG, MOLD_SLUG,
    MOLD_THREADS_SLUG, MakeInvocation, Sandbox, TargetState, combined, pinned_toolchain,
    write_with_old_mtime,
};
use variant::BenchVariant;

/// A cell holding a one-decimal duration, as `bench-build` formats them.
/// Timings are inherently unstable, so tests assert on shape, not value.
pub fn is_timing(cell: &str) -> bool {
    // Exactly one point, with digits either side. A looser check accepts `.`,
    // `..`, and `1.2.3`, none of which the benchmark can emit.
    let Some((whole, fraction)) = cell.split_once('.') else {
        return false;
    };
    !whole.is_empty()
        && !fraction.is_empty()
        && whole.chars().all(|c| c.is_ascii_digit())
        && fraction.chars().all(|c| c.is_ascii_digit())
}

pub const VARIANT_FLAGS: [(&str, &[&str]); 3] = [
    (DEFAULT_SLUG, &[]),
    (MOLD_SLUG, &["-Clink-arg=-fuse-ld=mold"]),
    (
        MOLD_THREADS_SLUG,
        &["-Zthreads=8", "-Clink-arg=-fuse-ld=mold"],
    ),
];

/// Check the recorded passes: their membership and pairing, each variant's own
/// contract, that variants measured separately, and where each pass sits
/// relative to the touch.
///
/// Nothing here depends on the order the variants ran in. The benchmark
/// shuffles them per sample precisely because a fixed order confounds the
/// variant with shared host state, so an assertion that pinned the order would
/// re-impose the defect the shuffle exists to remove. What must hold instead is
/// that every slug appears `2 * BENCH_REPEATS` times, and that each appearance
/// is a clean pass followed by an incremental one.
pub fn check_benchmark_invocations(
    invocations: &[CargoInvocation],
    baseline_mtime: i64,
) -> Result<()> {
    // Two builds — clean then incremental — for each variant in each sample.
    ensure!(
        invocations.len() == 2 * BENCH_SLUGS.len() * BENCH_REPEATS,
        "should measure two builds per variant per sample, recorded {}",
        invocations.len()
    );

    let (pairs, rest) = invocations.as_chunks::<2>();
    ensure!(
        rest.is_empty(),
        "passes should come in pairs, got {} spare",
        rest.len()
    );

    let toolchain = pinned_toolchain()?;
    let variants: Vec<BenchVariant<'_>> = pairs
        .iter()
        .map(|pair| {
            // Drop the sample index the slug carries, so a variant measured in
            // two samples is checked against one expectation rather than
            // needing a row per sample.
            let (_, slug) = pair[0].target_dir().rsplit_once('/').with_context(|| {
                format!(
                    "pass should run in a variant directory, got `{}`",
                    pair[0].target_dir()
                )
            })?;
            let (label, flags) = VARIANT_FLAGS
                .iter()
                .find(|(known, _)| *known == slug)
                .with_context(|| format!("`{slug}` is not a benchmarked variant"))?;
            Ok(BenchVariant::from_pair(label, pair, flags))
        })
        .collect::<Result<Vec<_>>>()?;

    for variant in &variants {
        variant.check(&toolchain)?;
    }

    // Every variant must be measured the same number of times. Otherwise a
    // shuffle could drop one from a sample and the table would still look
    // plausible while comparing unequal evidence.
    for slug in BENCH_SLUGS {
        let seen = variants
            .iter()
            .filter(|variant| variant.label == slug)
            .count();
        ensure!(
            seen == BENCH_REPEATS,
            "`{slug}` should be measured {BENCH_REPEATS} time(s), found {seen}"
        );
    }

    // Pairwise rather than against the first alone: two accelerated variants
    // sharing a directory would warm each other's cache and understate the
    // second, which a check against the baseline would not catch. Compared
    // across samples too, so an accidental per-sample directory cannot hide.
    for (index, variant) in variants.iter().enumerate() {
        for other in variants.iter().skip(index + 1) {
            ensure!(
                variant.target_dir() != other.target_dir() || variant.label == other.label,
                "`{}` and `{}` share a directory, got `{}`",
                variant.label,
                other.label,
                variant.target_dir()
            );
        }
    }

    // `first()` rather than an index: the pair count was asserted above, but an
    // indexing panic here would report a slice bound rather than the missing
    // measurement the assertion is about.
    let first = pairs
        .first()
        .context("the run should record at least one pair")?;
    check_touch_ordering(first, invocations, baseline_mtime)
}

/// Only the first pass of the whole run precedes any touch; the benchmark
/// touches the file between every variant's two passes, so every later pass
/// must see a newer timestamp. Comparing against a backdated baseline rather
/// than between passes keeps this free of filesystem timestamp granularity.
fn check_touch_ordering(
    first_pair: &[CargoInvocation],
    invocations: &[CargoInvocation],
    baseline_mtime: i64,
) -> Result<()> {
    let first = first_pair.first().context("expected a recorded pass")?;
    ensure!(
        first.touch_mtime() == Some(baseline_mtime),
        "the run's first clean pass should precede any touch, got {:?}",
        first.touch_mtime()
    );
    // Every pass after the run's very first follows a touch: the incremental
    // pass of the opening variant, and both passes of every variant after it.
    for (index, pass) in invocations.iter().enumerate().skip(1) {
        ensure!(
            pass.touch_mtime()
                .is_some_and(|mtime| mtime > baseline_mtime),
            "pass {index} should follow a touch, got {:?}",
            pass.touch_mtime()
        );
    }
    Ok(())
}

/// What a variant's target directory holds before the benchmark starts.
#[derive(Copy, Clone, Debug)]
enum PreState {
    /// Nothing there, as on a first run.
    Absent,
    /// The directory exists but is empty.
    Empty,
    /// The directory exists and holds an artefact from an earlier run.
    Populated,
}

impl PreState {
    fn stage(self, sandbox: &Sandbox, dir: &Utf8Path) -> Result<()> {
        match self {
            Self::Absent => Ok(()),
            Self::Empty => sandbox.create_dir(dir),
            Self::Populated => sandbox.write_file(&dir.join("stale-artefact"), "stale"),
        }
    }
}

fn pre_state_strategy() -> impl Strategy<Value = PreState> {
    prop_oneof![
        Just(PreState::Absent),
        Just(PreState::Empty),
        Just(PreState::Populated),
    ]
}

proptest! {
    // Sixteen draws. The matrix case runs a whole benchmark against a fake
    // Cargo, which is cheap but not free; the timing-shape properties below
    // draw short strings and cost almost nothing.
    #![proptest_config(ProptestConfig {
        cases: 16,
        // Name the file explicitly. The default `SourceParallel` policy
        // looks for a `lib.rs` or `main.rs` beside the source and gives up
        // in an integration-test crate, so recorded seeds were neither
        // written nor replayed — the file on disk was inert.
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/build_tools_bench_tests/checks.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// Whatever each variant's target directory held beforehand, every variant
    /// must record a clean pass then an incremental one.
    ///
    /// This is the invariant the `rm -rf` exists to provide: the benchmark's
    /// first measurement must not inherit a previous run's artefacts, or the
    /// "clean build" column measures something else entirely. Ranging over the
    /// prior states shows the wipe erases history rather than merely working on
    /// an empty sandbox.
    #[test]
    fn the_benchmark_wipes_whatever_each_variant_started_from(
        default_pre in pre_state_strategy(),
        mold_pre in pre_state_strategy(),
    ) {
        let fail = |error: anyhow::Error| TestCaseError::fail(error.to_string());
        let scenario = BuildScenario::prepare().map_err(fail)?;
        let sandbox = scenario.sandbox();

        let touch_file = sandbox.home().join("bench-touch");
        write_with_old_mtime(sandbox, &touch_file).map_err(fail)?;
        let bench_root = sandbox.home().join("bench");
        for (slug, pre) in BENCH_SLUGS.into_iter().zip([default_pre, mold_pre, mold_pre]) {
            pre.stage(sandbox, &bench_root.join(slug)).map_err(fail)?;
        }

        let invocation = MakeInvocation::new("bench-build")
            .variable("CARGO", scenario.cargo().executable())
            .environment("BENCH_ROOT", &bench_root)
            .environment("BENCH_TOUCH_FILE", &touch_file);
        let output = sandbox.run_make(&invocation).map_err(fail)?;
        prop_assert!(
            output.status.success(),
            "bench-build should succeed from {:?}/{:?}, got `{}`",
            default_pre,
            mold_pre,
            combined(&output)
        );

        let invocations = scenario.cargo().invocations().map_err(fail)?;
        // Every pass after a variant's clean one must find its directory in
        // place. Asserted as a property of the sequence rather than against a
        // fixed list, because the benchmark shuffles the variants per sample
        // and the order is not the contract — the clean/incremental pairing is.
        // A pair's members share a directory, so their states must differ.
        let states: Vec<TargetState> = invocations
            .iter()
            .map(CargoInvocation::target_state)
            .collect();
        for (index, pair) in states.as_chunks::<2>().0.iter().enumerate() {
            prop_assert_eq!(
                *pair,
                [TargetState::Absent, TargetState::Present],
                "pair {} should be a clean pass then an incremental one, from {:?}/{:?}",
                index,
                default_pre,
                mold_pre
            );
        }
        prop_assert_eq!(
            states.len(),
            2 * BENCH_SLUGS.len() * BENCH_REPEATS,
            "every variant should be measured in every sample, from {:?}/{:?}",
            default_pre,
            mold_pre
        );
    }

    /// A timing cell is exactly `<digits>.<digits>`. Generating around that
    /// shape covers the malformed neighbours — bare dots, multiple points, a
    /// missing side — that a hand-picked example list tends to miss.
    #[test]
    fn is_timing_accepts_exactly_one_point_between_digits(
        cell in r"[0-9.]{0,6}"
    ) {
        let expected = {
            let mut parts = cell.split('.');
            let whole = parts.next().unwrap_or_default();
            let fraction = parts.next().unwrap_or_default();
            parts.next().is_none()
                && cell.contains('.')
                && !whole.is_empty()
                && !fraction.is_empty()
        };
        prop_assert_eq!(is_timing(&cell), expected, "cell `{}`", cell);
    }

    /// Whatever the digits, a well-formed one-decimal timing is accepted.
    #[test]
    fn is_timing_accepts_any_one_decimal_duration(whole in 0u32..100_000, fraction in 0u32..10) {
        let cell = format!("{whole}.{fraction}");
        prop_assert!(is_timing(&cell), "cell `{}`", cell);
    }
}
