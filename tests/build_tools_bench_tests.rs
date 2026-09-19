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

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use proptest::prelude::*;
use proptest::proptest;
use proptest::test_runner::FileFailurePersistence;
use rstest::rstest;
use test_support::build_tools::{
    BENCH_REPEATS, BENCH_SLUGS, BenchFixture, BuildScenario, CargoInvocation, DEFAULT_SLUG,
    MOLD_SLUG, MOLD_THREADS_SLUG, MakeInvocation, Sandbox, TargetState, combined, pinned_toolchain,
    real_utility, write_with_old_mtime,
};

/// The flags each variant must hand Cargo through `RUSTFLAGS`.
///
/// The baseline's empty list is the point of the whole row: assigning
/// `RUSTFLAGS` to nothing is what displaces `.cargo/config.toml`'s tables and
/// restores the pre-standard build, so "no flags" here means "assigned and
/// empty", not "unset".
const VARIANT_FLAGS: [(&str, &[&str]); 3] = [
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
fn check_benchmark_invocations(invocations: &[CargoInvocation], baseline_mtime: i64) -> Result<()> {
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
    // Nine combinations; a few extra draws cost little because the fake Cargo
    // returns immediately.
    #![proptest_config(ProptestConfig {
        cases: 12,
        // Name the file explicitly. The default `SourceParallel` policy
        // looks for a `lib.rs` or `main.rs` beside the source and gives up
        // in an integration-test crate, so recorded seeds were neither
        // written nor replayed — the file on disk was inert.
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/build_tools_bench_tests.proptest-regressions",
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
}

/// A clean pass is only clean if Cargo's intermediates live under the directory
/// the harness removed.
///
/// Cargo can be told to keep them elsewhere, and a caller that has done so —
/// a shared build tree on a multi-agent host, say — would have every variant
/// share one directory. The `rm -rf` would then stop making the next pass
/// clean, and the table would report three warm builds while looking exactly
/// like three cold ones. Nothing about the output would give that away, which
/// is why it is asserted rather than assumed.
#[test]
fn the_benchmark_takes_back_a_redirected_build_directory() -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let fixture = BenchFixture::prepare(&scenario)?;
    let redirected = scenario.sandbox().home().join("shared-build-tree");

    let invocation = MakeInvocation::new("bench-build")
        .variable("CARGO", scenario.cargo().executable())
        .environment("BENCH_ROOT", &fixture.root)
        .environment("BENCH_TOUCH_FILE", &fixture.touch_file)
        .environment("CARGO_BUILD_BUILD_DIR", &redirected);
    let output = scenario.sandbox().run_make(&invocation)?;
    ensure!(
        output.status.success(),
        "bench-build should succeed, got `{}`",
        combined(&output)
    );

    for pass in scenario.cargo().invocations()? {
        ensure!(
            pass.build_dir().is_empty(),
            "the benchmark must drop an inherited build directory, got `{}`",
            pass.build_dir()
        );
    }
    Ok(())
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
        // Fail the first accelerated variant, after the baseline has already
        // touched the file. A non-empty RUSTFLAGS is what distinguishes an
        // accelerated variant from the baseline, which assigns it empty.
        sandbox.write_fake(
            &sandbox.bin(),
            "cargo",
            "[ -z \"${RUSTFLAGS:-}\" ] || exit 1\nexit 0",
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
fn bench_target_emits_every_variant_row() -> Result<()> {
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
        "| Default (platform linker) |",
        "| `mold` |",
        "| `mold`, parallel frontend |",
    ] {
        ensure!(
            stdout.contains(variant),
            "table should carry the `{variant}` row, got `{stdout}`"
        );
    }

    check_benchmark_invocations(&scenario.cargo().invocations()?, fixture.baseline_mtime)
}

/// An inherited `CARGO_ENCODED_RUSTFLAGS` must not reach a single measured build.
///
/// The sandbox clears the environment, so `BenchVariant::check`'s assertion that
/// the variable is unset would hold even for a script that never removed it —
/// there would be nothing to remove. This is the case that gives that assertion
/// its teeth: the harness exports a hostile value and the run must strip it.
///
/// It has to be stripped rather than emptied. Cargo reads the encoded variable
/// before `RUSTFLAGS` and takes the first source it finds, so an inherited value
/// survives as every variant compiling identically while the table reports three
/// different rows. A developer with `CARGO_ENCODED_RUSTFLAGS` exported — which
/// `cargo nextest` and sccache wrappers both set — would see a benchmark that
/// looked perfectly plausible and compared nothing.
#[test]
fn an_inherited_encoded_rustflags_never_reaches_a_measured_build() -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let fixture = BenchFixture::prepare(&scenario)?;

    let invocation = MakeInvocation::new("bench-build")
        .variable("CARGO", scenario.cargo().executable())
        .environment("BENCH_ROOT", &fixture.root)
        .environment("BENCH_TOUCH_FILE", &fixture.touch_file)
        .environment("CARGO_ENCODED_RUSTFLAGS", "-Dinherited=should-not-apply");
    let output = scenario.sandbox().run_make(&invocation)?;

    ensure!(
        output.status.success(),
        "make bench-build should succeed, got `{}`",
        combined(&output)
    );
    // The whole run is checked, not just the variable: stripping the encoded
    // value must not have been achieved by dropping the variants' own flags too,
    // which would satisfy the absence assertion on its own.
    check_benchmark_invocations(&scenario.cargo().invocations()?, fixture.baseline_mtime)
}

/// One variant's pair of recorded builds: the clean pass then the incremental.
struct BenchVariant<'a> {
    label: &'a str,
    clean: &'a CargoInvocation,
    incremental: &'a CargoInvocation,
    /// The flags this variant must hand Cargo, which is the only thing that
    /// distinguishes one row from another.
    flags: &'a [&'a str],
}

impl<'a> BenchVariant<'a> {
    /// Build a descriptor from the variant's recorded pair of passes.
    ///
    /// The fixed-size array makes the pairing a type-level fact, so there is no
    /// absent-pass case left to handle at runtime.
    const fn from_pair(
        label: &'a str,
        [clean, incremental]: &'a [CargoInvocation; 2],
        flags: &'a [&'a str],
    ) -> Self {
        Self {
            label,
            clean,
            incremental,
            flags,
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
            // Assigned, never inherited: an unset RUSTFLAGS would let
            // `.cargo/config.toml` decide the variant, and every row would
            // then measure the same build.
            let recorded: Vec<&str> = pass
                .rustflags()
                .with_context(|| format!("`{label}` must assign RUSTFLAGS"))?
                .split_whitespace()
                .collect();
            ensure!(
                recorded == self.flags,
                "`{label}` should pass {:?}, got {recorded:?}",
                self.flags
            );
            ensure!(
                pass.toolchain() == toolchain,
                "`{label}` should run under `{toolchain}`, got `{}`",
                pass.toolchain()
            );
            // Both compiler wrappers assigned and empty, for the same reason
            // RUSTFLAGS is assigned rather than inherited. A developer shell
            // on a shared host commonly exports a wrapper chaining to
            // `sccache`; with one in force the first clean pass fills the
            // cache and the rest read it back, so the table times cache
            // retrieval and the row order decides the winner. Unset is not
            // good enough as an expectation: only an assignment displaces an
            // exported value, and `RUSTC_WORKSPACE_WRAPPER` is honoured
            // separately, so clearing one alone still wraps the workspace's
            // own crates.
            ensure!(
                pass.wrappers_cleared(),
                "`{label}` should clear both compiler wrappers, got RUSTC_WRAPPER={:?} \
                 RUSTC_WORKSPACE_WRAPPER={:?}",
                pass.wrapper(),
                pass.workspace_wrapper()
            );
            // Unset, not merely empty. Cargo reads the encoded variable before
            // `RUSTFLAGS`, so an empty one would still displace every variant's
            // flags — the rows would all compile identically while the table
            // reported three different builds.
            ensure!(
                pass.encoded_rustflags().is_none(),
                "`{label}` must remove CARGO_ENCODED_RUSTFLAGS, found {:?}",
                pass.encoded_rustflags()
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
