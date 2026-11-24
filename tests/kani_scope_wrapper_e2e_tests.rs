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
//! Every evidence read therefore requires `LoadState=loaded` before trusting
//! `Result`, and the success scenario reads its `Result` only to report that
//! it is unavailable.
//!
//! # A payload's exit status is not the scope's `Result`
//!
//! The two are produced by different machinery and can disagree in both
//! directions, which was measured rather than reasoned about:
//!
//! - A payload that exits 7 leaves a scope the manager collects as a success.
//!   A scope has no main process, so nothing maps an exit status onto
//!   `Result=`.
//! - A payload that exits zero can leave a scope recorded as
//!   `Result=timeout`. The runtime deadline covers the whole control group,
//!   not the payload, so it can fire while the pipeline is still tearing down.
//!   Reproduced on the reference host by driving the cap down to 5 ms: some
//!   runs reported caller status 0 while the unit was retained as failed with
//!   `Result=timeout`.
//!
//! The second shape is the explanation for a failed `netsuke-kani-wrapper-*-ok`
//! unit left behind by an earlier run of this suite: the `-ok` in the name is
//! the scenario, not a claim about the scope, and a short cap on a loaded host
//! is enough to fail a scope whose payload exited zero. No wrapper flag was at
//! fault. The routine scenarios therefore use a wide runtime cap and never
//! assert a timing bound, while the timeout scenarios keep the short one.
//!
//! # Collection, and when not to collect
//!
//! `systemd-run --collect` is `-G`, a shortcut for
//! `CollectMode=inactive-or-failed`: the unit is unloaded once it finishes,
//! whether it succeeded or failed. The default, `CollectMode=inactive`, keeps
//! a failed unit loaded until someone resets it, which is how a host
//! accumulates failed scopes until `systemctl --user is-system-running`
//! reports `degraded`.
//!
//! Collection is right for a routine run and wrong for one whose evidence
//! lives in the stopped unit. Unloading discards the unit's execution results,
//! so a collected scope cannot answer `Result` at all. Each scenario therefore
//! declares its [`Retention`]:
//!
//! - [`Retention::Collect`] for the routine shape. The durable evidence is the
//!   caller's status, the capture, the unit identity and the journal, none of
//!   which depends on the unit staying loaded.
//! - [`Retention::Retain`] for the timeout scenarios, which read
//!   `Result=timeout` and the escalation entries from a unit that is still
//!   loaded, and for the investigation scenarios, which must be able to report
//!   a manager-assigned failure with its `Result` and journal rather than
//!   discovering that the unit has already vanished.
//!
//! # Teardown
//!
//! Every scenario runs inside a [`guard::ScopeGuard`]. The guard is created
//! before the unit is launched, so it exists on every exit path, and it tears
//! the unit down from [`Drop`] as well as from its fallible `finish`. An
//! injected error, an early `?` return and a panic therefore all leave the
//! unit stopped and reset, and cleanup always names the one unit the guard
//! owns — never a glob, and never `systemctl --user reset-failed` with no
//! argument.
//!
//! # What nextest cannot clean up
//!
//! Nextest terminates a test that overruns its allowance by signalling the
//! test's process group, which kills the test before `Drop` can run. A unit
//! that is active at that moment is left behind, and this suite cannot prevent
//! it: the process that would have cleaned up is gone. The consequence is
//! bounded — nextest's allowance for this suite is far above the sum of its
//! caps — but a run that was killed outright may still need
//! `systemctl --user reset-failed netsuke-kani-wrapper-<pid>-*.scope` by hand.
//! `docs/developers-guide.md` records that recovery, together with the reason
//! the prefix is the pid.

#![cfg(all(unix, target_os = "linux"))]

#[path = "kani_scope_wrapper_e2e_tests/evidence.rs"]
mod evidence;
#[path = "kani_scope_wrapper_e2e_tests/guard.rs"]
mod guard;
#[path = "kani_scope_wrapper_e2e_tests/harness.rs"]
mod harness;
#[path = "kani_scope_wrapper_e2e_tests/host.rs"]
mod host;
#[path = "kani_scope_wrapper_e2e_tests/manager.rs"]
mod manager;
#[path = "kani_scope_wrapper_e2e_tests/teardown_tests.rs"]
mod teardown_tests;
#[path = "kani_scope_wrapper_e2e_tests/timeout_tests.rs"]
mod timeout_tests;

use anyhow::{Result, ensure};

use evidence::Outcome;
use guard::{with_scope, with_unit};
use harness::{
    PAYLOAD_LIFETIME, ROUTINE_RUNTIME_CAP, RUNTIME_CAP, Retention, run_scoped,
    run_without_pipefail, unit_name,
};
use host::{prerequisites_met, skip};
use manager::{capture_exists, ensure_own_unit_settled, wait_until_unloaded};

/// A successful payload exits zero and its full output is captured.
///
/// The scope's own state is asserted only as "nothing failed". A scope which
/// succeeds is collected the moment it exits, so the caller's exit status and
/// the captured file are the evidence that outlives the unit; a `Result` read
/// from the absent unit would be the default systemd answers with.
#[test]
fn successful_payload_exits_zero_and_capture_is_complete() -> Result<()> {
    let scenario = "success";
    if !prerequisites_met(scenario)? {
        return Ok(());
    }
    with_scope(scenario, |scope| {
        let marker = format!("payload-marker-{}", std::process::id());
        let output = run_scoped(
            scope.unit(),
            ROUTINE_RUNTIME_CAP,
            Retention::Retain,
            &format!("echo {marker}; echo second-line; exit 0"),
        )?;
        let outcome = Outcome::harvest(scope.unit(), &output)?;
        ensure!(
            outcome.caller_success,
            "a successful payload should exit 0, got {:?} (stderr: {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
            outcome.describe()
        );
        ensure!(
            outcome.capture.contains(&marker),
            "the capture should hold the payload's stdout: {}",
            outcome.describe()
        );
        ensure!(
            outcome.capture.contains("second-line"),
            "the capture should hold every line the payload wrote: {}",
            outcome.describe()
        );
        outcome.ensure_no_failed_scope("a payload that exited zero must not leave a failed scope")
    })
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
    if !prerequisites_met(scenario)? {
        return Ok(());
    }
    with_scope(scenario, |scope| {
        let output = run_scoped(
            scope.unit(),
            ROUTINE_RUNTIME_CAP,
            Retention::Retain,
            "echo verifier-failed; exit 7",
        )?;
        let outcome = Outcome::harvest(scope.unit(), &output)?;
        ensure!(
            outcome.caller_code == Some(7),
            "a failing verifier piped through tee should reach the caller as 7, got {}: {}",
            outcome.caller_description(),
            outcome.describe()
        );
        ensure!(
            outcome.capture.contains("verifier-failed"),
            "a failing payload's output should still be captured: {}",
            outcome.describe()
        );
        outcome.ensure_no_failed_scope(
            "a failing payload must reach the caller without leaving a failed scope",
        )
    })
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
    if !prerequisites_met(scenario)? {
        return Ok(());
    }
    with_scope(scenario, |scope| {
        let output = run_without_pipefail(scope.unit(), ROUTINE_RUNTIME_CAP)?;
        let outcome = Outcome::harvest(scope.unit(), &output)?;
        ensure!(
            outcome.caller_success,
            "without pipefail the pipeline status is tee's, so the caller should see 0, got {}: {}",
            outcome.caller_description(),
            outcome.describe()
        );
        ensure!(
            outcome.capture.contains("masked-failure"),
            "the masked failure should still reach the capture: {}",
            outcome.describe()
        );
        outcome.ensure_no_failed_scope("the masked control must not leave a failed scope")
    })
}

/// A collected scope unloads even though its payload was stopped, and its
/// journal still explains why.
///
/// This is the routine shape the developer guide prescribes: `--collect` means
/// a failure cannot accumulate, so nothing is left for a later reset. The
/// unit's own `Result` is deliberately not asserted — collection removes it —
/// and the runtime limit is established from the journal instead, which is
/// exactly the durable-evidence contract the guide documents.
#[test]
fn collected_scope_unloads_after_a_stopped_payload() -> Result<()> {
    let scenario = "collected-timeout";
    if !prerequisites_met(scenario)? {
        return Ok(());
    }
    let unit = unit_name(scenario);
    with_unit(unit.clone(), |scope| {
        let output = run_scoped(
            scope.unit(),
            RUNTIME_CAP,
            Retention::Collect,
            &format!("trap '' TERM; sleep {}", PAYLOAD_LIFETIME.as_secs()),
        )?;
        // The journal's escalation entry is written when the grace lapses, so
        // the unit has to be gone before the extract is complete.
        wait_until_unloaded(scope.unit())?;
        let outcome = Outcome::harvest(scope.unit(), &output)?;

        ensure!(
            !outcome.caller_success,
            "a payload stopped at the deadline should not report success to the caller: {}",
            outcome.describe()
        );
        ensure!(
            capture_exists(scope.unit())?,
            "the capture should exist even though the unit was collected: {}",
            outcome.describe()
        );
        if outcome.journal.is_empty() {
            return skip(
                scenario,
                "the per-user journal is empty or unreadable for this unit",
            );
        }
        ensure!(
            outcome
                .journal
                .text()
                .contains("Scope reached runtime time limit"),
            "the journal should record the runtime limit that stopped the payload: {}",
            outcome.describe()
        );
        ensure!(
            outcome.journal.text().contains("Stopping timed out")
                && outcome.journal.text().contains("SIGKILL"),
            "the journal should record the escalation to SIGKILL: {}",
            outcome.describe()
        );
        outcome.ensure_no_failed_scope("a collected scope must never be left failed")
    })?;
    ensure_own_unit_settled(&unit, "a collected scope must leave no failed unit behind")
}

/// A collected scope still reports the payload's non-zero status to the caller.
///
/// Collection must not weaken status propagation: the caller's code is the
/// evidence a routine run relies on, because the unit itself is gone by the
/// time anyone could query it.
#[test]
fn collected_scope_propagates_a_failing_status_without_accumulating() -> Result<()> {
    let scenario = "collected-failure";
    if !prerequisites_met(scenario)? {
        return Ok(());
    }
    let unit = unit_name(scenario);
    with_unit(unit.clone(), |scope| {
        let output = run_scoped(
            scope.unit(),
            ROUTINE_RUNTIME_CAP,
            Retention::Collect,
            "echo verifier-failed; exit 7",
        )?;
        wait_until_unloaded(scope.unit())?;
        let outcome = Outcome::harvest(scope.unit(), &output)?;

        ensure!(
            outcome.caller_code == Some(7),
            "a collected run must still report the verifier's status 7, got {}: {}",
            outcome.caller_description(),
            outcome.describe()
        );
        ensure!(
            outcome.capture.contains("verifier-failed"),
            "the complete capture must survive collection: {}",
            outcome.describe()
        );
        if outcome.journal.is_empty() {
            return skip(
                scenario,
                "the per-user journal is empty or unreadable for this unit",
            );
        }
        ensure!(
            !outcome.journal.text().contains("Failed with result"),
            "a payload that exited non-zero must not leave the scope itself failed: {}",
            outcome.describe()
        );
        outcome.ensure_no_failed_scope("a collected scope must never be left failed")
    })?;
    ensure_own_unit_settled(&unit, "a collected scope must leave no failed unit behind")
}
