//! The wrapper's runtime bound: the deadline, the stop grace, and the journal.
//!
//! These scenarios are the ones that keep their unit loaded after it stops.
//! A scope's `Result` and its journal are unreadable once the unit is
//! collected, so each of these runs with
//! [`super::harness::Retention::Retain`] and reads the stopped unit before
//! teardown resets it. The routine scenarios live in the crate root and
//! `--collect`; the teardown scenarios live in [`super::teardown_tests`].
//!
//! The bound asserted here is the pair, not `RuntimeMaxSec` alone: reaching
//! the deadline starts a stop, and `TimeoutStopSec` is what bounds it. A
//! payload that ignores `SIGTERM` is therefore stopped at the sum, which is
//! the property `stop_grace_is_additive_to_the_runtime_cap` measures.

use anyhow::{Result, ensure};

use super::{
    evidence::Outcome,
    guard::with_unit,
    harness::{
        PAYLOAD_LIFETIME, RUNTIME_CAP, Retention, STOP_GRACE, TIMING_SLACK, run_scoped, unit_name,
    },
    host::{prerequisites_met, skip},
    manager::{ensure_own_unit_settled, wait_for_terminal},
};

/// A payload that ignores `SIGTERM` is stopped after the grace period.
///
/// This is the scenario the wrapper's `TimeoutStopSec` exists for. The payload
/// traps `TERM` and sleeps far past the deadline, so it can only be stopped by
/// the scope: the runtime cap fires, the stop grace lapses, and systemd
/// escalates to `SIGKILL`. The bound is asserted against the scope's lifetime
/// rather than the caller's, because the caller returns when the launcher dies
/// — which for this piped shape is at the deadline, well before the grace has
/// elapsed. See the module docs.
#[test]
fn sigterm_ignoring_payload_is_stopped_after_the_grace_period() -> Result<()> {
    let scenario = "sigterm-ignoring";
    if !prerequisites_met(scenario)? {
        return Ok(());
    }
    let unit = unit_name(scenario);
    with_unit(unit.clone(), |scope| {
        let output = run_scoped(
            scope.unit(),
            RUNTIME_CAP,
            Retention::Retain,
            &format!("trap '' TERM; sleep {}", PAYLOAD_LIFETIME.as_secs()),
        )?;
        let lifetime = wait_for_terminal(scope.unit())?;
        let outcome = Outcome::harvest(scope.unit(), &output)?;

        // The payload cannot have finished on its own: it was told to sleep
        // far longer than the bound, and it ignores the polite signal.
        let expected_ceiling = RUNTIME_CAP + STOP_GRACE + TIMING_SLACK;
        ensure!(
            lifetime < expected_ceiling,
            "a SIGTERM-ignoring payload should be stopped within {expected_ceiling:?}, \
             but the scope lived {lifetime:?}: {}",
            outcome.describe()
        );
        ensure!(
            lifetime >= RUNTIME_CAP,
            "the payload should have reached the runtime cap of {RUNTIME_CAP:?} before \
             being stopped, but the scope lived only {lifetime:?}: {}",
            outcome.describe()
        );
        ensure!(
            !outcome.caller_success,
            "a payload killed at the deadline should not report success to the caller: {}",
            outcome.describe()
        );

        let (active, result) = outcome.loaded_state()?;
        ensure!(
            result == "timeout",
            "the scope should record a timeout, got Result={result:?} (ActiveState={active:?}): {}",
            outcome.describe()
        );
        Ok(())
    })?;
    ensure_own_unit_settled(
        &unit,
        "the timeout scenario's own failed unit must be reset",
    )
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
    if !prerequisites_met(scenario)? {
        return Ok(());
    }
    let unit = unit_name(scenario);
    with_unit(unit.clone(), |scope| {
        let output = run_scoped(
            scope.unit(),
            RUNTIME_CAP,
            Retention::Retain,
            &format!("trap '' TERM; sleep {}", PAYLOAD_LIFETIME.as_secs()),
        )?;
        let lifetime = wait_for_terminal(scope.unit())?;
        let outcome = Outcome::harvest(scope.unit(), &output)?;

        // The bound is the *sum*. A nested grace stops the payload at the cap;
        // an additive one carries it to cap plus grace. The tolerance has to
        // stay below the smallest grace reduction this test must catch, or the
        // assertion cannot distinguish a shortened grace from scheduling
        // delay: half a grace (1.5s) would accept a grace cut to 1.5s once the
        // stop overshoots. A quarter of the grace — 750ms against 3s — rejects
        // that while still absorbing the sub-second granularity measured on the
        // reference host. The scope stops a little *after* the sum, never
        // before it, so the tolerance only ever absorbs overshoot.
        let expected_floor = RUNTIME_CAP + STOP_GRACE;
        let tolerance = STOP_GRACE / 4;
        // The tolerance is half the grace, so this cannot underflow for any
        // grace short enough to be meaningful; `saturating_sub` states that
        // rather than panicking on a configuration the constants exclude.
        let minimum = expected_floor.saturating_sub(tolerance);
        ensure!(
            lifetime > minimum,
            "the stop grace should be additive: the payload should live about \
             {expected_floor:?} (cap {RUNTIME_CAP:?} plus grace {STOP_GRACE:?}), but the \
             scope lived only {lifetime:?} — which is what a nested or shortened grace \
             would look like: {}",
            outcome.describe()
        );

        let (_, result) = outcome.loaded_state()?;
        ensure!(
            result == "timeout",
            "the scope should record a timeout, got {result:?}: {}",
            outcome.describe()
        );
        Ok(())
    })?;
    ensure_own_unit_settled(
        &unit,
        "the timeout scenario's own failed unit must be reset",
    )
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
    if !prerequisites_met(scenario)? {
        return Ok(());
    }
    let unit = unit_name(scenario);
    with_unit(unit.clone(), |scope| {
        let output = run_scoped(
            scope.unit(),
            RUNTIME_CAP,
            Retention::Retain,
            &format!("trap '' TERM; sleep {}", PAYLOAD_LIFETIME.as_secs()),
        )?;
        let _ = wait_for_terminal(scope.unit())?;
        let mut outcome = Outcome::harvest(scope.unit(), &output)?;
        // Wait for the entries this test asserts on. A successful `--sync`
        // makes the first read final; a refused one does not, and the final
        // stop entries may still be arriving.
        outcome.settle_journal(&[
            "Scope reached runtime time limit",
            "Stopping timed out. Killing.",
            "with signal SIGKILL",
        ]);

        // The journal is the only record that survives collection, so an
        // unreadable one is a skip rather than a failure: a host may run
        // without a persistent or readable user journal even though the scope
        // itself worked.
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
            "the journal should record the scope reaching its runtime limit: {}",
            outcome.describe()
        );
        // The escalation sentence, not two independent substrings. systemd also
        // emits "Stopping timed out. Skipping SIGKILL." when `SendSIGKILL` is
        // disabled, which contains both "Stopping timed out" and "SIGKILL" and
        // would satisfy a two-substring check while proving the opposite — that
        // no kill happened. The positive message measured on the reference host
        // is "Stopping timed out. Killing.", followed by the per-process
        // "Killing process <pid> (<name>) with signal SIGKILL." line.
        ensure!(
            outcome
                .journal
                .text()
                .contains("Stopping timed out. Killing."),
            "the journal should record the stop timing out and escalating, which systemd \
             writes as `Stopping timed out. Killing.`; a `Skipping SIGKILL` variant would \
             mean the grace period did not escalate at all: {}",
            outcome.describe()
        );
        ensure!(
            outcome.journal.text().contains("with signal SIGKILL"),
            "the journal should name the signal actually sent to the process: {}",
            outcome.describe()
        );
        Ok(())
    })?;
    ensure_own_unit_settled(
        &unit,
        "the timeout scenario's own failed unit must be reset",
    )
}
