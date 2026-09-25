//! End-to-end coverage for the documented Kani scope wrapper.
//!
//! `docs/execplans/4-2-2-kani-harnesses-for-cycle-canonicalization.md` and
//! `docs/execplans/4-2-3-kani-harnesses-for-command-interpolation.md` document
//! a `systemd-run --user --scope` wrapper whose runtime bound is
//! `-p RuntimeMaxSec=` paired with `-p TimeoutStopSec=`, and whose capture is
//! `tee` *inside* the scope with `set -o pipefail` in the same shell. This
//! suite exercises that wrapper for real, at test-sized time limits.
//!
//! It exists because the properties the wrapper claims are not provable by
//! reading it. A transient scope that has been collected answers any property
//! query with the defaults, `Result=success` among them, so `systemctl show`
//! on a finished unit can read `success` for a run that was in fact killed at
//! its deadline. See "Reading scope evidence" below for the two shapes this
//! actually takes and for the guard that catches each.
//!
//! # Host prerequisites
//!
//! - systemd 254 or later. `RuntimeMaxSec` on a scope needs 244 and
//!   `--expand-environment=no` needs 254; the higher floor binds.
//! - A running per-user systemd manager, reachable as `systemctl --user`.
//!   This is what `--user --scope` delegates the cgroup to.
//! - Delegated cgroup support, so the user manager may create a scope.
//!
//! A host missing any of these cannot run the wrapper at all. This suite
//! reports that as a skip naming the missing prerequisite, never as a pass:
//! a green result here means the wrapper was really exercised.
//!
//! # Reading scope evidence
//!
//! Three traps make a naive read wrong, and all three were measured on the
//! reference host while writing this suite.
//!
//! 1. `systemctl show` resolves a bare name as a `.service`, so a scope must
//!    be queried as `<name>.scope`.
//! 2. Querying a unit that does not exist is not an error: systemd answers
//!    with the defaults for the requested properties. `LoadState=not-found`
//!    is the only tell.
//! 3. A scope that *succeeded* is collected the moment it exits, so this is
//!    not a rare race but the normal outcome for the success scenario. A
//!    scope that *failed* at its deadline stays loaded, which is why the
//!    timeout scenarios can read `Result` at all.
//!
//! Every evidence read below therefore asserts `LoadState=loaded` before
//! trusting `Result`, and the success scenario deliberately reads no scope
//! property — its unit is gone, and the caller's status plus the capture are
//! the evidence that survives.
//!
//! # Why the timing bound is taken from the scope
//!
//! The obvious way to time the stop is to time the `systemd-run` call, and it
//! is wrong. The caller returns when the launcher dies, and that happens at
//! different points depending on how the payload is shaped: a bare payload
//! returns at deadline plus grace, while the documented pipe leaves the
//! launcher killed at the deadline itself. Both were measured; the caller's
//! return is therefore not a bound on the work. The scope's own lifetime is,
//! so the grace is measured by polling the unit to a terminal state.

#![cfg(all(unix, target_os = "linux"))]

use std::{
    process::{Command, Output},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};

/// The runtime bound used by every scenario.
///
/// Test-sized rather than the documented eight minutes: the wrapper's
/// behaviour depends on the ratio between this and [`STOP_GRACE`], not on
/// their magnitude.
const RUNTIME_CAP: Duration = Duration::from_secs(2);

/// The stop grace period paired with [`RUNTIME_CAP`].
///
/// The wrapper's effective bound is the sum, so this is also what makes the
/// additive-grace assertion measurable.
const STOP_GRACE: Duration = Duration::from_secs(3);

/// How long a scenario's payload would run if nothing stopped it.
///
/// Comfortably longer than `RUNTIME_CAP + STOP_GRACE`, so a payload that
/// finishes on its own can never be mistaken for one the scope stopped.
const PAYLOAD_LIFETIME: Duration = Duration::from_secs(60);

/// Slack allowed above the nominal bound before a scenario is judged overrun.
///
/// A scope's stop is not instantaneous: the deadline expires, the stop job
/// runs, and a non-cooperative payload is killed once the grace period lapses.
/// Observed overshoot on the reference host was under a second.
const TIMING_SLACK: Duration = Duration::from_secs(4);

/// How long to wait for a scope to leave its active states.
///
/// A generous ceiling: it only ever elapses when the wrapper is broken, and
/// the scenarios it guards are bounded by `RUNTIME_CAP + STOP_GRACE`.
const TERMINAL_CEILING: Duration = Duration::from_secs(45);

/// A unique unit name for one scenario.
///
/// Includes the process id so a re-run, or a concurrent invocation from
/// another worktree, cannot collide on a transient unit name.
fn unit_name(scenario: &str) -> String {
    format!("netsuke-kani-wrapper-{}-{scenario}", std::process::id())
}

/// Run the wrapper exactly as the plans document it, with test-sized limits.
///
/// `payload` is the shell body placed inside the scope. The resource
/// properties, `nice` level, and capture placement mirror the documented
/// command; only the two time limits are reduced.
fn run_scoped(unit: &str, payload: &str, capture: &str) -> Result<Output> {
    let log = format!("/tmp/{capture}");
    // The payload is brace-grouped so that the pipeline's left side is the
    // whole group. Without the braces a trailing `exit` in the payload would
    // end the shell before `tee` ever ran, leaving an empty capture and a
    // pipeline status that says nothing about the payload.
    let body = format!("set -o pipefail; {{ {payload}; }} 2>&1 | tee {log}");
    let output = Command::new("systemd-run")
        .args([
            "--user",
            "--scope",
            "--expand-environment=no",
            "--unit",
            unit,
            "-p",
            &format!("RuntimeMaxSec={}s", RUNTIME_CAP.as_secs()),
            "-p",
            &format!("TimeoutStopSec={}s", STOP_GRACE.as_secs()),
            "-p",
            "CPUQuota=200%",
            "-p",
            "MemoryMax=8G",
            "-p",
            "MemorySwapMax=0",
            "-p",
            "TasksMax=96",
            "-p",
            "IOWeight=20",
            "/usr/bin/nice",
            "-n",
            "15",
            "bash",
            "-c",
            &body,
        ])
        .output()
        .context("run the documented systemd-run scope wrapper")?;
    Ok(output)
}

/// Read one systemd property for a scope.
///
/// The unit name is passed with its `.scope` suffix because a bare name is
/// resolved as a service and would never match. `LoadState` is checked
/// separately by [`scope_evidence`], since an absent unit answers with
/// defaults rather than failing.
fn scope_property(unit: &str, property: &str) -> Result<String> {
    let output = Command::new("systemctl")
        .args(["--user", "show", "-P", property, &format!("{unit}.scope")])
        .output()
        .with_context(|| format!("read {property} for {unit}.scope"))?;
    ensure!(
        output.status.success(),
        "systemctl show {property} {unit}.scope failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Poll a scope until it is no longer active, returning its lifetime.
///
/// `is-active` reports `deactivating` while the stop job is still running,
/// which is *not* terminal: the grace period has not yet elapsed. Returning at
/// `deactivating` would measure the deadline and call it the bound, which is
/// precisely how a missing `TimeoutStopSec` would go unnoticed. Only `failed`
/// and `inactive` end the wait.
fn wait_for_terminal(unit: &str) -> Result<Duration> {
    let started = Instant::now();
    loop {
        let state = scope_property(unit, "ActiveState")?;
        if matches!(state.as_str(), "failed" | "inactive") {
            return Ok(started.elapsed());
        }
        ensure!(
            started.elapsed() < TERMINAL_CEILING,
            "{unit}.scope was still {state} after {TERMINAL_CEILING:?}"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

/// The scope's `(LoadState, ActiveState, Result)`, with `LoadState` enforced.
///
/// A query against a unit that does not exist silently returns property
/// defaults, `Result=success` among them. Requiring `LoadState=loaded` is what
/// separates real evidence from that trap; a collected unit fails here rather
/// than being read as a success.
fn scope_evidence(unit: &str) -> Result<(String, String, String)> {
    let load = scope_property(unit, "LoadState")?;
    ensure!(
        load == "loaded",
        "{unit}.scope should still be loaded so its Result is real evidence, \
         got LoadState={load} — an unloaded unit answers with defaults, so any \
         Result read from it would be fabricated"
    );
    Ok((
        load,
        scope_property(unit, "ActiveState")?,
        scope_property(unit, "Result")?,
    ))
}

/// Read the payload's captured output from `/tmp/<capture>`.
///
/// The directory is opened as a capability rather than reached through
/// `std::fs`, which this repository forbids: the capture path is fixed, so the
/// handle is the whole of the surface this needs.
fn captured(capture: &str) -> Result<String> {
    let tmp = Dir::open_ambient_dir("/tmp", ambient_authority())
        .context("open /tmp to read the captured output")?;
    tmp.read_to_string(capture)
        .with_context(|| format!("read the captured output from /tmp/{capture}"))
}

/// Best-effort teardown so a rerun starts from a clean unit name.
///
/// The status is deliberately discarded: a unit that was never loaded has
/// nothing to reset, so `reset-failed` legitimately fails on the first run of a
/// fresh scenario. `drop` consumes the result rather than binding it, which
/// keeps the `#[must_use]` on `Output` satisfied without an underscore binding.
fn reset_failed(unit: &str) {
    drop(
        Command::new("systemctl")
            .args(["--user", "reset-failed", &format!("{unit}.scope")])
            .output(),
    );
}

/// Check the three host prerequisites and report what is missing.
///
/// Returns `Ok(None)` when the host can run the wrapper, or `Ok(Some(reason))`
/// naming the first unmet prerequisite. The caller reports that as a visible
/// skip; it must never be folded into a pass.
fn missing_prerequisite() -> Result<Option<String>> {
    // The user manager is the prerequisite `--user --scope` actually needs.
    let manager = Command::new("systemctl")
        .args(["--user", "is-system-running"])
        .output()
        .context("probe for a per-user systemd manager")?;
    let state = String::from_utf8_lossy(&manager.stdout).trim().to_owned();
    // `degraded` and `running` both mean the manager is up and can accept a
    // scope; only `offline` (or a missing binary) means it is not.
    if !matches!(state.as_str(), "running" | "degraded" | "starting") {
        return Ok(Some(format!(
            "no running per-user systemd manager (systemctl --user reported `{state}`)"
        )));
    }

    let version = Command::new("systemctl")
        .arg("--version")
        .output()
        .context("read the systemd version")?;
    let text = String::from_utf8_lossy(&version.stdout);
    let major = text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|raw| raw.split('-').next())
        .and_then(|raw| raw.parse::<u32>().ok());
    // The arm binds `found`, not `major` or `version`: either name is already
    // taken by an enclosing binding in this function, and this repository
    // denies both `clippy::shadow-reuse` and `clippy::shadow-unrelated`.
    match major {
        Some(found) if found >= 254 => {}
        Some(found) => {
            return Ok(Some(format!(
                "systemd {found} is below the wrapper's floor of 254"
            )));
        }
        None => {
            return Ok(Some(
                "could not read the systemd version from `systemctl --version`".to_owned(),
            ));
        }
    }

    // Delegation: the user manager may only create a scope if the controllers
    // it needs are delegated down the tree. A cgroup v1 host fails here.
    let cgroup = Dir::open_ambient_dir("/sys/fs/cgroup", ambient_authority())
        .context("open /sys/fs/cgroup")?;
    let delegated = cgroup
        .read_to_string("cgroup.controllers")
        .context("read /sys/fs/cgroup/cgroup.controllers")?;
    ensure!(
        !delegated.trim().is_empty(),
        "/sys/fs/cgroup/cgroup.controllers should list delegated controllers"
    );
    Ok(None)
}

/// Whether an unprovable scenario should fail the run instead of skipping.
///
/// A host without the prerequisites cannot exercise the wrapper at all, and
/// skipping is the honest report there. It is not honest in the dedicated CI
/// lane, where a suite that skipped everywhere would read as green coverage
/// while proving nothing, so that lane sets this variable and a skip becomes a
/// failure. Without it, a runner that quietly lacks a per-user manager would
/// leave the wrapper untested and unnoticed.
#[expect(
    clippy::disallowed_methods,
    reason = "the dedicated CI lane signals strict mode through the environment; this is the single site that reads it"
)]
fn strict_mode() -> bool {
    std::env::var_os("NETSUKE_KANI_SCOPE_WRAPPER_STRICT").is_some_and(|value| value == "1")
}

/// Report a skip for an unprovable scenario, and end it.
///
/// Under [`strict_mode`] the same condition is a failure rather than a skip,
/// so a lane that requires real coverage cannot be satisfied by an absent
/// prerequisite.
#[expect(
    clippy::print_stderr,
    reason = "test harness: an unavailable prerequisite must be visible in the captured test output instead of passing silently"
)]
fn skip(scenario: &str, reason: &str) -> Result<()> {
    ensure!(
        !strict_mode(),
        "{scenario} could not be exercised: {reason} (strict mode requires the \
         wrapper to be tested for real, so this is a failure rather than a skip)"
    );
    eprintln!("skipped: {scenario} — {reason}");
    Ok(())
}

/// A successful payload exits zero and its full output is captured.
///
/// No scope property is asserted here, and that is deliberate: a scope which
/// succeeds is collected the moment it exits, so a read would return
/// `LoadState=not-found` and a fabricated `Result=success`. The caller's exit
/// status and the captured file are the evidence that outlives the unit.
#[test]
fn successful_payload_exits_zero_and_capture_is_complete() -> Result<()> {
    let scenario = "success";
    if let Some(reason) = missing_prerequisite()? {
        skip(scenario, &reason)?;
        return Ok(());
    }
    let unit = unit_name("ok");
    let capture = format!("{unit}.out");
    reset_failed(&unit);

    let marker = format!("payload-marker-{}", std::process::id());
    let output = run_scoped(
        &unit,
        &format!("echo {marker}; echo second-line; exit 0"),
        &capture,
    )?;
    ensure!(
        output.status.success(),
        "a successful payload should exit 0, got {:?}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let text = captured(&capture)?;
    ensure!(
        text.contains(&marker),
        "the capture should hold the payload's stdout, got `{text}`"
    );
    ensure!(
        text.contains("second-line"),
        "the capture should hold every line the payload wrote, got `{text}`"
    );
    Ok(())
}

/// A failing payload's status survives the `tee` pipeline.
///
/// `make` failing inside the scope must reach the caller. Without
/// `set -o pipefail` in the same shell the pipeline's status is `tee`'s and
/// the failure is masked; this asserts both directions so a regression that
/// drops `pipefail` cannot pass.
#[test]
fn failing_payload_propagates_through_tee_with_pipefail() -> Result<()> {
    let scenario = "failing-payload";
    if let Some(reason) = missing_prerequisite()? {
        skip(scenario, &reason)?;
        return Ok(());
    }
    let unit = unit_name("fail");
    let capture = format!("{unit}.out");
    reset_failed(&unit);

    let output = run_scoped(&unit, "echo verifier-failed; exit 7", &capture)?;
    ensure!(
        output.status.code() == Some(7),
        "a failing verifier piped through tee should reach the caller as 7, got {:?}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let text = captured(&capture)?;
    ensure!(
        text.contains("verifier-failed"),
        "a failing payload's output should still be captured, got `{text}`"
    );
    reset_failed(&unit);
    Ok(())
}

/// The control for the pipefail assertion: without it the failure is masked.
///
/// This is the half that makes the previous test meaningful. If a payload's
/// non-zero status reached the caller regardless, asserting that it does would
/// prove nothing about `pipefail`. The body is the documented one minus the
/// `pipefail`, so the two differ in exactly the property under test.
#[test]
fn failing_payload_is_masked_without_pipefail() -> Result<()> {
    let scenario = "masked-without-pipefail";
    if let Some(reason) = missing_prerequisite()? {
        skip(scenario, &reason)?;
        return Ok(());
    }
    let unit = unit_name("masked");
    let capture = format!("{unit}.out");
    reset_failed(&unit);

    let log = format!("/tmp/{capture}");
    let body = format!("{{ echo masked-failure; exit 7; }} 2>&1 | tee {log}");
    let output = Command::new("systemd-run")
        .args([
            "--user",
            "--scope",
            "--expand-environment=no",
            "--unit",
            &unit,
            "-p",
            &format!("RuntimeMaxSec={}s", RUNTIME_CAP.as_secs()),
            "-p",
            &format!("TimeoutStopSec={}s", STOP_GRACE.as_secs()),
            "bash",
            "-c",
            &body,
        ])
        .output()
        .context("run the wrapper without pipefail")?;
    ensure!(
        output.status.success(),
        "without pipefail the pipeline status is tee's, so the caller should see 0, got {:?}",
        output.status
    );
    reset_failed(&unit);
    Ok(())
}

/// A payload that ignores `SIGTERM` is stopped after the grace period.
///
/// This is the scenario the wrapper's `TimeoutStopSec` exists for. The
/// payload traps `TERM` and sleeps far past the deadline, so it can only be
/// stopped by the scope: the runtime cap fires, the stop grace lapses, and
/// systemd escalates to `SIGKILL`. The bound is asserted against the scope's
/// lifetime rather than the caller's, because the caller returns when the
/// launcher dies — which for this piped shape is at the deadline, well before
/// the grace has elapsed. See the module docs.
#[test]
fn sigterm_ignoring_payload_is_stopped_after_the_grace_period() -> Result<()> {
    let scenario = "sigterm-ignoring";
    if let Some(reason) = missing_prerequisite()? {
        skip(scenario, &reason)?;
        return Ok(());
    }
    let unit = unit_name("kill");
    let capture = format!("{unit}.out");
    reset_failed(&unit);

    let output = run_scoped(
        &unit,
        &format!("trap '' TERM; sleep {}", PAYLOAD_LIFETIME.as_secs()),
        &capture,
    )?;
    let lifetime = wait_for_terminal(&unit)?;

    // The payload cannot have finished on its own: it was told to sleep far
    // longer than the bound, and it ignores the polite signal.
    let expected_ceiling = RUNTIME_CAP + STOP_GRACE + TIMING_SLACK;
    ensure!(
        lifetime < expected_ceiling,
        "a SIGTERM-ignoring payload should be stopped within {expected_ceiling:?} \
         (cap {RUNTIME_CAP:?} plus grace {STOP_GRACE:?}), but the scope lived {lifetime:?}"
    );
    ensure!(
        lifetime >= RUNTIME_CAP,
        "the payload should have reached the runtime cap of {RUNTIME_CAP:?} before \
         being stopped, but the scope lived only {lifetime:?}"
    );
    ensure!(
        !output.status.success(),
        "a payload killed at the deadline should not report success to the caller, got {:?}",
        output.status
    );

    let (_, active, result) = scope_evidence(&unit)?;
    ensure!(
        result == "timeout",
        "the scope should record a timeout, got Result={result:?} (ActiveState={active:?})"
    );
    reset_failed(&unit);
    Ok(())
}

/// The stop grace is additive to the runtime cap, not nested inside it.
///
/// This is the property that makes `TimeoutStopSec` load-bearing. If the grace
/// were nested, a `SIGTERM`-ignoring payload would still stop at
/// `RuntimeMaxSec`; because it is additive, the same payload stops at the sum.
/// The assertion is relational rather than absolute, so it holds whatever the
/// two test-sized limits are.
#[test]
fn stop_grace_is_additive_to_the_runtime_cap() -> Result<()> {
    let scenario = "additive-grace";
    if let Some(reason) = missing_prerequisite()? {
        skip(scenario, &reason)?;
        return Ok(());
    }
    let unit = unit_name("additive");
    let capture = format!("{unit}.out");
    reset_failed(&unit);

    let _ = run_scoped(
        &unit,
        &format!("trap '' TERM; sleep {}", PAYLOAD_LIFETIME.as_secs()),
        &capture,
    )?;
    let lifetime = wait_for_terminal(&unit)?;

    // A nested grace would stop the payload at the cap. An additive one
    // carries the stop past it, so the scope must outlive `RUNTIME_CAP` on its
    // own — no assumption about the caller, and no dependence on the slack.
    ensure!(
        lifetime > RUNTIME_CAP,
        "an additive stop grace should carry the stop past the {RUNTIME_CAP:?} cap, \
         but the scope lived only {lifetime:?} — which is what a nested grace, or a \
         `TimeoutStopSec` that never took effect, would look like"
    );

    let (_, _, result) = scope_evidence(&unit)?;
    ensure!(
        result == "timeout",
        "the scope should record a timeout, got {result:?}"
    );
    reset_failed(&unit);
    Ok(())
}

/// The journal names the runtime limit and the escalation to `SIGKILL`.
///
/// A scope's `Result` says it timed out; the journal says *how* it was
/// stopped. This asserts the stop journey the wrapper depends on — deadline
/// reached, polite stop timed out, kill — so a change that removed
/// `TimeoutStopSec` and let the host's 90-second default apply would be
/// visible here even though `Result` stayed `timeout`. The journal is read
/// only after the scope reaches a terminal state, since the escalation is
/// written when the grace lapses rather than when the caller returns.
#[test]
fn journal_records_the_runtime_limit_and_the_kill() -> Result<()> {
    let scenario = "journal-evidence";
    if let Some(reason) = missing_prerequisite()? {
        skip(scenario, &reason)?;
        return Ok(());
    }
    let unit = unit_name("journal");
    let capture = format!("{unit}.out");
    reset_failed(&unit);

    let _ = run_scoped(
        &unit,
        &format!("trap '' TERM; sleep {}", PAYLOAD_LIFETIME.as_secs()),
        &capture,
    )?;
    let _ = wait_for_terminal(&unit)?;

    let output = Command::new("journalctl")
        .args([
            "--user",
            "-u",
            &format!("{unit}.scope"),
            "--no-pager",
            "-o",
            "cat",
            "-n",
            "40",
        ])
        .output()
        .context("read the scope's journal")?;
    ensure!(
        output.status.success(),
        "journalctl for {unit}.scope failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);

    // The journal is the only record that survives collection, so an unreadable
    // one is a skip rather than a failure: a host may run without a persistent
    // or readable user journal even though the scope itself worked.
    if text.trim().is_empty() {
        skip(
            scenario,
            "the per-user journal is empty or unreadable for this unit",
        )?;
        reset_failed(&unit);
        return Ok(());
    }

    ensure!(
        text.contains("Scope reached runtime time limit"),
        "the journal should record the scope reaching its runtime limit, got:\n{text}"
    );
    ensure!(
        text.contains("Stopping timed out") && text.contains("SIGKILL"),
        "the journal should record the stop timing out and escalating to SIGKILL \
         (the grace period's job), got:\n{text}"
    );
    reset_failed(&unit);
    Ok(())
}
