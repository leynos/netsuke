//! Properties a recorded benchmark run must satisfy whatever it started from.
//!
//! Split from [`super`] along the same seam the rest of this repository uses:
//! the parent holds the checks a *particular* run is held to, and this holds the
//! ones stated over a range of starting states. They are a different kind of
//! claim — the parent's assertions are about what this run did, and these are
//! about what every run must do — so they read better apart, and the parent
//! stays within the Whitaker `module_max_lines` cap.
//!
//! The seeds live in `checks.proptest-regressions` beside the parent — the name
//! it carries because the parent used to hold these properties — and are named
//! explicitly rather than left to the default policy. That policy looks for a
//! `lib.rs` or `main.rs` beside the source and gives up in an integration-test
//! crate, which is how the file came to be inert.

use super::{BENCH_REPEATS, BENCH_SLUGS, is_timing};
use camino::Utf8Path;
use proptest::prelude::*;
use proptest::proptest;
use proptest::test_runner::{FileFailurePersistence, TestCaseError};
use test_support::build_tools::{
    BuildScenario, CargoInvocation, MakeInvocation, Sandbox, TargetState, combined,
    write_with_old_mtime,
};

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
    fn stage(self, sandbox: &Sandbox, dir: &Utf8Path) -> Result<(), anyhow::Error> {
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
        // written nor replayed — the file on disk was inert. `Direct` takes
        // the path verbatim and resolves it against the process's working
        // directory, not against this file, so the seeds stay reachable from
        // the parent modules too.
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
