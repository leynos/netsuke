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
use camino::Utf8Path;
use proptest::prelude::*;
use proptest::proptest;
use proptest::test_runner::FileFailurePersistence;
use rstest::rstest;
use test_support::dev_fast::{
    BenchFixture, BuildScenario, CargoInvocation, DEFAULT_SLUG, DEV_FAST_CONFIG_PATH,
    DEV_FAST_SLUG, MakeInvocation, Sandbox, TargetState, combined, pinned_toolchain, real_utility,
    write_with_old_mtime,
};

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

/// The touched file's timestamp must be put back, whether the run finishes or
/// aborts partway.
///
/// The benchmark touches a tracked source to make its second pass incremental.
/// Leaving it newer than `target/` would make the developer's next ordinary
/// build redo work for reasons nothing on screen explains — and a failed run is
/// the case most likely to leave it that way, so it is the one worth asserting.
#[rstest]
#[case::completes(true)]
#[case::aborts(false)]
fn the_touched_file_is_restored_however_the_run_ends(#[case] succeeds: bool) -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let sandbox = scenario.sandbox();
    let fixture = BenchFixture::prepare(&scenario)?;

    if !succeeds {
        // Fail the second variant, after the first has already touched the file.
        sandbox.write_fake(
            &sandbox.bin(),
            "cargo",
            "case \"$*\" in *--config*) exit 1 ;; *) exit 0 ;; esac",
        )?;
    }

    let invocation = MakeInvocation::new("bench-build")
        .variable("CARGO", scenario.cargo().executable())
        .environment("BENCH_ROOT", &fixture.root)
        .environment("BENCH_TOUCH_FILE", &fixture.touch_file);
    let output = sandbox.run_make(&invocation)?;
    ensure!(
        output.status.success() == succeeds,
        "the run should {} , got `{}`",
        if succeeds { "succeed" } else { "abort" },
        combined(&output)
    );

    ensure!(
        sandbox.mtime_seconds(&fixture.touch_file)? == fixture.baseline_mtime,
        "the touched file should be restored to {}, found {}",
        fixture.baseline_mtime,
        sandbox.mtime_seconds(&fixture.touch_file)?
    );
    Ok(())
}

/// When the restore itself fails, the run must say so.
///
/// This is the one case where silence costs the most: the developer keeps a
/// source file newer than the build outputs, so every later build redoes work,
/// and has nothing on screen connecting that to the benchmark they ran.
#[test]
fn a_failed_restore_warns_rather_than_passing_silently() -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let sandbox = scenario.sandbox();
    let fixture = BenchFixture::prepare(&scenario)?;

    // A `touch` that captures the stamp and then refuses to put it back. Only
    // the second `-r` fails, because the first is the capture the script needs
    // to get as far as a restore at all. Delegating to the real binary keeps
    // every other use — the touches between passes — behaving normally.
    let real_touch = real_utility("touch")?;
    let marker = sandbox.home().join("stamp-captured");
    sandbox.write_fake(
        &sandbox.bin(),
        "touch",
        &format!(
            "if [ \"$1\" = -r ]; then\n\
             \x20 [ -e '{marker}' ] && exit 1\n\
             \x20 : >'{marker}'\n\
             fi\n\
             exec '{real_touch}' \"$@\""
        ),
    )?;

    let invocation = MakeInvocation::new("bench-build")
        .variable("CARGO", scenario.cargo().executable())
        .environment("BENCH_ROOT", &fixture.root)
        .environment("BENCH_TOUCH_FILE", &fixture.touch_file);
    let text = combined(&sandbox.run_make(&invocation)?);

    ensure!(
        text.contains("failed to restore the timestamp"),
        "a failed restore should be reported, got `{text}`"
    );
    ensure!(
        text.contains(fixture.touch_file.as_str()),
        "the warning should name the file left newer, got `{text}`"
    );
    Ok(())
}

#[test]
fn bench_target_emits_both_variant_rows() -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let fixture = BenchFixture::prepare(&scenario)?;

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
