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
//!
//! The checks a recorded run is held to — the variant table, the variant
//! descriptor, and the pass-by-pass contract — live in [`checks`](checks), and
//! the cases about the *order* the variants ran in, with what decides it, live
//! in [`order_cases`](order_cases). This file holds the rest of the
//! observable-behaviour cases.

#![cfg(all(unix, target_os = "linux"))]

#[path = "build_tools_bench_tests/checks.rs"]
mod checks;
#[path = "build_tools_bench_tests/order_cases.rs"]
mod order_cases;

use anyhow::{Result, ensure};
use rstest::rstest;
use test_support::build_tools::{
    BENCH_REPEATS, BENCH_SLUGS, BenchFixture, BuildScenario, MakeInvocation, combined, real_utility,
};

use checks::{check_recorded_run, is_timing};

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
        // Fail an accelerated variant, and only once the baseline has passed.
        //
        // A non-empty `RUSTFLAGS` is what identifies an accelerated variant:
        // the baseline assigns it empty. Failing on that alone is not enough,
        // because the variants are shuffled and the first one measured may be
        // accelerated. That variant would abort on its clean pass, before the
        // script had touched the file, and the assertion below would then pass
        // for a reason with nothing to do with restoring anything: with nothing
        // touched, `BENCH_TOUCH_STAMP` stays empty and the restore never runs.
        //
        // So the baseline's clean pass writes a marker, and an accelerated
        // variant fails only when it finds one. Nothing can fail before that
        // marker exists, so the run always reaches the touch — including when
        // the draw leads with an accelerated variant, whose clean pass now
        // succeeds and is followed by the touch its incremental pass needs. The
        // abort then always finds a touched file to put back.
        let marker = sandbox.home().join("baseline-built");
        sandbox.write_fake(
            &sandbox.bin(),
            "cargo",
            &format!(
                "[ -z \"${{RUSTFLAGS:-}}\" ] && {{ : >'{marker}'; exit 0; }}\n\
                 [ -e '{marker}' ] || exit 0\n\
                 exit 1"
            ),
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

    check_recorded_run(
        &stdout,
        &scenario.cargo().invocations()?,
        fixture.baseline_mtime,
    )
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
    let stdout = String::from_utf8_lossy(&output.stdout);
    check_recorded_run(
        &stdout,
        &scenario.cargo().invocations()?,
        fixture.baseline_mtime,
    )
}

/// The table's shape: one row per variant per repeat, every cell a timing.
///
/// `bench_target_emits_every_variant_row` already asserts the three captions,
/// and `check_benchmark_invocations` the recorded passes. What is left is the
/// *reported* count: each variant is measured `BENCH_REPEATS` times in a freshly
/// shuffled order, and each measurement is printed rather than folded into the
/// last. Asserting a bare number would pass only until the repeat count moved,
/// and then assert the old count rather than the invariant.
#[test]
fn the_table_reports_one_row_per_variant_per_repeat() -> Result<()> {
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
    ensure!(
        stdout.contains("| Variant | Clean build (s) | Incremental build (s) |"),
        "should emit the table header, got `{stdout}`"
    );
    let rows: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with("| `") || line.starts_with("| Default"))
        .collect();
    // One row per variant *per repeat*: the benchmark measures every variant
    // `BENCH_REPEATS` times in a freshly shuffled order so that page-cache
    // warmth spreads across the variants instead of pinning to whichever ran
    // first, and each measurement is reported rather than folded into the last.
    // Asserting a bare `3` would pass only until the repeat count changed, and
    // would then be asserting the old count rather than the invariant.
    let expected_rows = BENCH_SLUGS.len() * BENCH_REPEATS;
    ensure!(
        rows.len() == expected_rows,
        "should report {expected_rows} rows, one per variant per repeat, got `{stdout}`"
    );
    for row in rows {
        let measurements = row.split('|').map(str::trim).filter(|cell| is_timing(cell));
        ensure!(
            measurements.count() == 2,
            "row should carry two decimal timings, got `{row}`"
        );
    }
    Ok(())
}
