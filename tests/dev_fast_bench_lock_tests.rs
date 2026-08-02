//! Behavioural tests for the benchmark's exclusion lock.
//!
//! The benchmark deletes and rebuilds shared target directories and rewrites a
//! tracked source file's timestamp, so two runs in one checkout would corrupt
//! each other's measurements and leave the source permanently newer. The lock
//! is what makes a run indivisible; these tests pin down that it is taken
//! before any of that mutation happens, that it is released however the run
//! ends, and that a rejected run leaves the holder's state alone.

#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Result, ensure};
use rstest::rstest;
use test_support::dev_fast::{BenchFixture, BuildScenario, MakeInvocation, combined};

/// Build the standard benchmark invocation for a staged fixture.
fn bench_invocation(scenario: &BuildScenario, fixture: &BenchFixture) -> MakeInvocation {
    MakeInvocation::new("bench-build")
        .variable("CARGO", scenario.cargo().executable())
        .environment("BENCH_ROOT", &fixture.root)
        .environment("BENCH_TOUCH_FILE", &fixture.touch_file)
}

/// A held lock turns a concurrent run into a refusal with a remedy, rather than
/// letting it interleave and produce a plausible-looking but corrupt table.
#[test]
fn a_held_lock_rejects_a_concurrent_run() -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let fixture = BenchFixture::prepare(&scenario)?;
    // Stand in for a run already in progress.
    scenario.sandbox().create_dir(&fixture.lock_dir())?;

    let output = scenario
        .sandbox()
        .run_make(&bench_invocation(&scenario, &fixture))?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "a contended benchmark should refuse to run, got `{text}`"
    );
    ensure!(
        text.contains("another benchmark run holds"),
        "the refusal should name the cause, got `{text}`"
    );
    ensure!(
        text.contains("remove that directory"),
        "the refusal should name the remedy for a stale lock, got `{text}`"
    );
    Ok(())
}

/// The refusal must be inert: a run that loses the race must not have already
/// touched the source file or wiped the holder's warm target directories, and
/// must not release a lock it never took.
#[test]
fn a_rejected_run_leaves_the_holders_state_untouched() -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let sandbox = scenario.sandbox();
    let fixture = BenchFixture::prepare(&scenario)?;
    let lock = fixture.lock_dir();
    sandbox.create_dir(&lock)?;
    // A warm cache the holder is mid-measurement on.
    let warm = fixture.root.join("default/warm-artefact");
    sandbox.write_file(&warm, "")?;

    let output = sandbox.run_make(&bench_invocation(&scenario, &fixture))?;
    ensure!(
        !output.status.success(),
        "the run should be rejected, got `{}`",
        combined(&output)
    );

    ensure!(
        lock.as_std_path().exists(),
        "the rejected run must not release a lock it never took"
    );
    ensure!(
        warm.as_std_path().exists(),
        "the holder's warm target directory must survive a rejected run"
    );
    ensure!(
        sandbox.mtime_seconds(&fixture.touch_file)? == fixture.baseline_mtime,
        "the rejected run must not have touched the source file"
    );
    ensure!(
        scenario.cargo().invocations()?.is_empty(),
        "the rejected run must not reach Cargo"
    );
    Ok(())
}

/// However a run ends, it must hand the lock back — otherwise the first
/// interrupted benchmark wedges every later one behind a stale directory.
#[rstest]
#[case::completes(true)]
#[case::aborts(false)]
fn the_lock_is_released_however_the_run_ends(#[case] succeeds: bool) -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let sandbox = scenario.sandbox();
    let fixture = BenchFixture::prepare(&scenario)?;

    if !succeeds {
        // Fail the accelerated variant, after the lock has been taken.
        sandbox.write_fake(
            &sandbox.bin(),
            "cargo",
            "case \"$*\" in *--config*) exit 1 ;; *) exit 0 ;; esac",
        )?;
    }

    let output = sandbox.run_make(&bench_invocation(&scenario, &fixture))?;
    ensure!(
        output.status.success() == succeeds,
        "the run should {}, got `{}`",
        if succeeds { "succeed" } else { "abort" },
        combined(&output)
    );
    ensure!(
        !fixture.lock_dir().as_std_path().exists(),
        "the lock should be released so the next run is not wedged behind it"
    );
    Ok(())
}

/// A released lock is not merely absent at the end — the next run can actually
/// take it. This is the property a developer cares about after an interrupted
/// benchmark, and it is not implied by the release assertion alone: a leftover
/// non-empty lock directory would fail `rmdir` silently and still refuse.
#[test]
fn a_later_run_succeeds_after_an_earlier_one_aborts() -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let sandbox = scenario.sandbox();
    let fixture = BenchFixture::prepare(&scenario)?;
    // A cargo that aborts the first run and then behaves, so the second run is
    // a genuine success rather than a differently-worded failure.
    let marker = sandbox.home().join("first-run-aborted");
    sandbox.write_fake(
        &sandbox.bin(),
        "cargo",
        &format!(
            "[ -e '{marker}' ] && exit 0\n\
             case \"$*\" in *--config*) : >'{marker}'; exit 1 ;; *) exit 0 ;; esac"
        ),
    )?;
    let first = sandbox.run_make(&bench_invocation(&scenario, &fixture))?;
    ensure!(
        !first.status.success(),
        "the first run should abort, got `{}`",
        combined(&first)
    );

    let second = sandbox.run_make(&bench_invocation(&scenario, &fixture))?;
    ensure!(
        second.status.success(),
        "the second run should not be blocked by the first, got `{}`",
        combined(&second)
    );
    Ok(())
}
