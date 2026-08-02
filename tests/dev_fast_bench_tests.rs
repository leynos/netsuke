//! Behavioural tests for `make bench-build`.
//!
//! The benchmark's job is to produce comparable figures, so what these tests
//! pin down is comparability: each variant measures in its own target
//! directory, a clean pass really starts from nothing, and the incremental pass
//! that follows really is incremental. A fake `cargo` records the target
//! directory and whether it already existed, which makes those facts checkable
//! rather than assumed.
//!
//! Timings themselves are never asserted — only their format. Every case is
//! hermetic: no network, and no real mold, rustup, or Cargo.

#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Context, Result, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use proptest::prelude::*;
use proptest::proptest;
use proptest::test_runner::FileFailurePersistence;
use std::time::{Duration, UNIX_EPOCH};
use test_support::dev_fast::{
    BuildScenario, CargoInvocation, DEV_FAST_CONFIG_PATH, MakeInvocation, Sandbox, TargetState,
    combined, pinned_toolchain,
};

/// Timestamp stamped on the benchmark's touch file before the run, chosen far
/// enough in the past that any `touch` is unambiguously newer. Comparing
/// against a fixed baseline keeps the assertion deterministic, where comparing
/// the two passes to each other would depend on filesystem timestamp
/// granularity.
const BASELINE_MTIME: i64 = 1_600_000_000;

/// Target-directory slugs the benchmark uses, one per variant.
const DEFAULT_SLUG: &str = "default";
const DEV_FAST_SLUG: &str = "dev-fast";

/// Create the touch file with [`BASELINE_MTIME`], returning that timestamp.
fn write_with_old_mtime(sandbox: &Sandbox, path: &Utf8Path) -> Result<i64> {
    let baseline = UNIX_EPOCH + Duration::from_secs(BASELINE_MTIME.unsigned_abs());
    sandbox.write_file_with_mtime(path, "", baseline)?;
    Ok(BASELINE_MTIME)
}

/// The disposable inputs one benchmark run needs.
///
/// Named fields rather than a returned tuple: `root` and `touch_file` are both
/// paths, so positional returns could be transposed at the call site without
/// the compiler noticing.
struct BenchFixture {
    /// Benchmark root; each variant gets its own target directory beneath it.
    root: Utf8PathBuf,
    /// The file the benchmark touches between a variant's two passes.
    touch_file: Utf8PathBuf,
    /// The touch file's timestamp before the run, for ordering assertions.
    baseline_mtime: i64,
}

/// Stage a benchmark run's disposable inputs.
///
/// The touch file stands in for `src/main.rs`, so a run does not invalidate the
/// working tree's build cache. Both target directories are seeded to model a
/// re-run: on a fresh sandbox the benchmark's `rm -rf` would be
/// indistinguishable from doing nothing, and the clean-pass assertion would
/// hold vacuously.
fn prepare_bench_fixture(scenario: &BuildScenario) -> Result<BenchFixture> {
    let sandbox = scenario.sandbox();
    let touch_file = sandbox.home().join("bench-touch");
    let baseline_mtime = write_with_old_mtime(sandbox, &touch_file)?;

    let root = sandbox.home().join("bench");
    for slug in [DEFAULT_SLUG, DEV_FAST_SLUG] {
        sandbox.create_dir(&root.join(slug))?;
    }
    Ok(BenchFixture {
        root,
        touch_file,
        baseline_mtime,
    })
}

/// Check the recorded passes: their count and pairing, each variant's own
/// contract, that the variants measured separately, and where each pass sits
/// relative to the touch.
fn check_benchmark_invocations(invocations: &[CargoInvocation], baseline_mtime: i64) -> Result<()> {
    // Four builds: clean and incremental, for each of the two variants.
    ensure!(
        invocations.len() == 4,
        "should measure two builds per variant, recorded {}",
        invocations.len()
    );

    // The benchmark measures the default variant first, then the accelerated
    // one, each as a clean pass followed by an incremental pass.
    let (pairs, rest) = invocations.as_chunks::<2>();
    ensure!(
        rest.is_empty(),
        "passes should come in pairs, got {} spare",
        rest.len()
    );
    let [default_chunk, dev_fast_chunk] = pairs else {
        bail!("expected one pair per variant, got {}", pairs.len());
    };
    let default_pass = BenchVariant::from_pair(DEFAULT_SLUG, default_chunk, false);
    let dev_fast_pass = BenchVariant::from_pair(DEV_FAST_SLUG, dev_fast_chunk, true);
    for variant in [&default_pass, &dev_fast_pass] {
        variant.check(&pinned_toolchain()?)?;
    }

    ensure!(
        default_pass.target_dir() != dev_fast_pass.target_dir(),
        "variants must not share a target directory, got `{}` and `{}`",
        default_pass.target_dir(),
        dev_fast_pass.target_dir()
    );

    check_touch_ordering(invocations, baseline_mtime)
}

/// Only the very first pass runs before any touch; each variant touches the
/// file between its own two passes, so every later pass must see a newer
/// timestamp. Comparing against a backdated baseline rather than between passes
/// keeps this free of filesystem timestamp granularity.
fn check_touch_ordering(invocations: &[CargoInvocation], baseline_mtime: i64) -> Result<()> {
    let first = invocations.first().context("expected a recorded pass")?;
    ensure!(
        first.touch_mtime() == Some(baseline_mtime),
        "the first clean pass should precede any touch, got {:?}",
        first.touch_mtime()
    );
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
    // Nine combinations; a few extra draws cost little because the fake Cargo
    // returns immediately.
    #![proptest_config(ProptestConfig {
        cases: 12,
        // Name the file explicitly. The default `SourceParallel` policy
        // looks for a `lib.rs` or `main.rs` beside the source and gives up
        // in an integration-test crate, so recorded seeds were neither
        // written nor replayed — the file on disk was inert.
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/dev_fast_bench_tests.proptest-regressions",
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
        dev_fast_pre in pre_state_strategy(),
    ) {
        let fail = |error: anyhow::Error| TestCaseError::fail(error.to_string());
        let scenario = BuildScenario::prepare().map_err(fail)?;
        let sandbox = scenario.sandbox();

        let touch_file = sandbox.home().join("bench-touch");
        write_with_old_mtime(sandbox, &touch_file).map_err(fail)?;
        let bench_root = sandbox.home().join("bench");
        for (slug, pre) in [(DEFAULT_SLUG, default_pre), (DEV_FAST_SLUG, dev_fast_pre)] {
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
            dev_fast_pre,
            combined(&output)
        );

        let invocations = scenario.cargo().invocations().map_err(fail)?;
        let states: Vec<TargetState> = invocations.iter().map(CargoInvocation::target_state).collect();
        prop_assert_eq!(
            states,
            vec![
                TargetState::Absent,
                TargetState::Present,
                TargetState::Absent,
                TargetState::Present,
            ],
            "each variant should measure a clean then an incremental pass, from {:?}/{:?}",
            default_pre,
            dev_fast_pre
        );
    }
}

#[test]
fn bench_target_emits_both_variant_rows() -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let fixture = prepare_bench_fixture(&scenario)?;

    let invocation = MakeInvocation::new("bench-build")
        .variable("CARGO", scenario.cargo().executable())
        .environment("BENCH_ROOT", &fixture.root)
        .environment("BENCH_TOUCH_FILE", &fixture.touch_file);
    let output = scenario.sandbox().run_make(&invocation)?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    ensure!(
        output.status.success(),
        "make bench-build should succeed, got `{}`",
        combined(&output)
    );
    for variant in [
        "| Default (LLVM, platform linker) |",
        "| dev-fast (Cranelift,",
    ] {
        ensure!(
            stdout.contains(variant),
            "table should carry the `{variant}` row, got `{stdout}`"
        );
    }

    check_benchmark_invocations(&scenario.cargo().invocations()?, fixture.baseline_mtime)
}

/// One variant's pair of recorded builds: the clean pass then the incremental.
struct BenchVariant<'a> {
    label: &'a str,
    clean: &'a CargoInvocation,
    incremental: &'a CargoInvocation,
    /// Whether this variant is the accelerated one, which alone carries the
    /// Cranelift fragment and the pinned toolchain.
    expects_fragment: bool,
}

impl<'a> BenchVariant<'a> {
    /// Build a descriptor from the variant's recorded pair of passes.
    ///
    /// The fixed-size array makes the pairing a type-level fact, so there is no
    /// absent-pass case left to handle at runtime.
    const fn from_pair(
        label: &'a str,
        [clean, incremental]: &'a [CargoInvocation; 2],
        expects_fragment: bool,
    ) -> Self {
        Self {
            label,
            clean,
            incremental,
            expects_fragment,
        }
    }

    fn target_dir(&self) -> &str {
        self.clean.target_dir()
    }

    fn check(&self, toolchain: &str) -> Result<()> {
        let label = self.label;
        for pass in [self.clean, self.incremental] {
            ensure!(
                pass.contains_sequence(&["build", "--bin", "netsuke"]),
                "`{label}` should build the binary, got `{:?}`",
                pass.arguments()
            );
            ensure!(
                pass.contains_sequence(&["--config", DEV_FAST_CONFIG_PATH])
                    == self.expects_fragment,
                "`{label}` fragment expectation ({}) not met, got `{:?}`",
                self.expects_fragment,
                pass.arguments()
            );
            let expected_toolchain = if self.expects_fragment { toolchain } else { "" };
            ensure!(
                pass.toolchain() == expected_toolchain,
                "`{label}` should run under `{expected_toolchain}`, got `{}`",
                pass.toolchain()
            );
        }

        ensure!(
            self.clean.target_dir() == self.incremental.target_dir(),
            "`{label}` should reuse one target directory across its passes"
        );
        ensure!(
            self.clean.target_dir().ends_with(label),
            "`{label}` should measure in its own directory, got `{}`",
            self.clean.target_dir()
        );

        // The harness removes the directory before the clean pass, and the
        // clean pass leaves it behind, so this ordering is what distinguishes
        // a genuine clean/incremental pair from two identical builds.
        ensure!(
            self.clean.target_state() == TargetState::Absent,
            "`{label}` clean pass should start from an empty target directory, got {:?}",
            self.clean.target_state()
        );
        ensure!(
            self.incremental.target_state() == TargetState::Present,
            "`{label}` incremental pass should reuse the clean pass's output, got {:?}",
            self.incremental.target_state()
        );

        Ok(())
    }
}
