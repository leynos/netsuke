//! Ownership of one scenario's transient unit and its teardown.
//!
//! A scenario that calls `reset_failed` only on its normal path is one failed
//! assertion away from leaving a failed unit behind: an early `?` return or a
//! panic skips every trailing statement. On a host that has accumulated those,
//! `systemctl --user is-system-running` reports `degraded`, and the failed
//! units persist until someone resets them by hand.
//!
//! [`ScopeGuard`] closes that gap. The guard is created before the scope is
//! launched, so it exists on every path the body can take, and [`Drop`] is the
//! one teardown path a panic cannot bypass. The two paths are not
//! interchangeable, and the split mirrors `AppliedPatch` in
//! `tests/kani_mutation_evidence_tests/applied_patch.rs`:
//!
//! - [`ScopeGuard::finish`] makes a failed teardown a test failure, so a run
//!   cannot report success with its unit still failed.
//! - [`Drop`] reports a teardown failure instead of raising it, because a
//!   panic is already unwinding and replacing its message would hide the
//!   defect that caused it.
//!
//! # What teardown may touch
//!
//! Exactly one unit, always by its exact name, and only when the manager says
//! it exists. A wildcard is used for read-only queries and never for `stop` or
//! `reset-failed`: the units a glob would match belong to other runs, and a
//! blanket reset would hide the very accumulation this suite exists to detect.
//!
//! Teardown reads `LoadState` first and leaves an unloaded unit alone —
//! `reset-failed` on a unit that is not loaded fails with "Unit ... not
//! loaded", so calling it unconditionally would turn a clean run into a
//! failure. A unit that is still active is stopped first, which is bounded by
//! the scope's own `TimeoutStopSec`; the wait that follows is bounded by
//! [`SETTLE_CEILING`].

use std::{
    process::Command,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};

use super::{
    harness::{STOP_GRACE, capture_path, unit_name},
    manager::{ScopeSnapshot, is_busy, remove_capture, scope_snapshot},
};

/// How long teardown waits for a stopped unit to reach a settled state.
///
/// A stop is bounded by the scope's `TimeoutStopSec`, so this adds one grace
/// period of slack for the manager's own bookkeeping.
const SETTLE_CEILING: Duration = Duration::from_secs(4 * STOP_GRACE.as_secs());

/// The interval between property reads while waiting for a settled state.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// One scenario's unit, torn down on every exit path.
pub(super) struct ScopeGuard {
    /// The exact unit name this guard owns, without the `.scope` suffix.
    unit: String,
    /// True while teardown is still owed.
    ///
    /// Cleared by [`Self::finish`] and by the logging-only path so the `Drop`
    /// fallback does not run a second teardown, which would report a spurious
    /// error on every successful scenario.
    armed: bool,
}

impl ScopeGuard {
    /// Take ownership of `unit`, which must not be launched before this runs.
    pub(super) fn named(unit: String) -> Self {
        Self { unit, armed: true }
    }

    /// The unit name this guard owns, without the `.scope` suffix.
    pub(super) fn unit(&self) -> &str {
        &self.unit
    }

    /// The capture path the payload writes, derived from the owned unit.
    pub(super) fn capture_path(&self) -> String {
        capture_path(&self.unit)
    }

    /// Tear the unit down, propagating a teardown or cleanup failure.
    ///
    /// A failed teardown keeps the capture: the capture is the evidence a
    /// developer needs to explain why the unit would not settle, and deleting
    /// it would discard the only artefact that outlived the failure.
    pub(super) fn finish(mut self) -> Result<()> {
        let outcome = self.teardown();
        self.armed = false;
        outcome?;
        remove_capture(&self.unit)
    }

    /// Stop and reset the owned unit, then wait for it to settle.
    ///
    /// Nothing is attempted against a unit that is not loaded. A stop is only
    /// issued for a unit the manager says is still running, and a reset only
    /// for one it says is failed: an `inactive` unit has no failure to clear,
    /// and the manager unloads it on its own schedule.
    fn teardown(&self) -> Result<()> {
        let Some(snapshot) = scope_snapshot(&self.unit)? else {
            return Ok(());
        };
        if is_busy(&snapshot.active) {
            systemctl_verb("stop", &self.unit)?;
        }
        // The stop under `--collect` may already have unloaded the unit, and a
        // stop issued against a failed unit can change its state, so the
        // decision to reset is taken from a fresh read rather than the first.
        match scope_snapshot(&self.unit)? {
            Some(state) if state.active == "failed" => {
                systemctl_verb("reset-failed", &self.unit)?;
            }
            _ => {}
        }
        wait_until_settled(&self.unit)
    }

    /// Run teardown without raising, for the panic path.
    fn teardown_logging_only(&mut self) {
        if !self.armed {
            return;
        }
        if let Err(err) = self.teardown() {
            tracing::error!(
                unit = %self.unit,
                %err,
                "failed to tear down a test scope; it may need `systemctl --user reset-failed` by hand"
            );
        }
        self.armed = false;
    }
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        self.teardown_logging_only();
    }
}

/// Run `body` with a freshly named unit, tearing the unit down afterwards.
///
/// The closure receives the guard rather than a bare name so a scenario cannot
/// launch a unit the guard does not own. On a body error the teardown is still
/// attempted, the capture is kept, and the body's own error is returned with
/// the capture path attached: a cleanup failure must never replace the defect
/// that caused the failure.
pub(super) fn with_scope<F>(scenario: &str, body: F) -> Result<()>
where
    F: FnOnce(&ScopeGuard) -> Result<()>,
{
    with_unit(unit_name(scenario), body)
}

/// Run `body` with a caller-chosen unit name, tearing the unit down afterwards.
///
/// This exists for the scenario that proves a name can be reused: a second
/// launch under the same name only succeeds if the first teardown really
/// unloaded the unit.
pub(super) fn with_unit<F>(unit: String, body: F) -> Result<()>
where
    F: FnOnce(&ScopeGuard) -> Result<()>,
{
    let guard = ScopeGuard::named(unit);
    match body(&guard) {
        Ok(()) => guard.finish(),
        Err(error) => {
            let capture = guard.capture_path();
            drop(guard);
            Err(error.context(format!("capture retained at {capture}")))
        }
    }
}

/// Run one `systemctl --user` verb against the owned unit, by exact name.
///
/// `stop` waits for the stop job, which the scope's own `TimeoutStopSec`
/// bounds, so this cannot outlast the grace period by much.
///
/// A unit can unload between the state read that decided to act and the
/// command that acts: collection is asynchronous, and a scope that finishes in
/// that window is gone. systemd reports that as `Unit ... not loaded`, and it
/// is success here rather than failure — the state the caller wanted is the
/// state the manager reached on its own. Every other refusal is an error, so a
/// genuinely refused stop or reset still fails the scenario.
fn systemctl_verb(verb: &str, unit: &str) -> Result<()> {
    let output = Command::new("systemctl")
        .args(["--user", verb, &format!("{unit}.scope")])
        .output()
        .with_context(|| format!("run systemctl --user {verb} {unit}.scope"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("not loaded"),
        "systemctl --user {verb} {unit}.scope failed: {}",
        stderr.trim()
    );
    Ok(())
}

/// Wait until the owned unit is neither busy nor failed.
///
/// An unloaded unit is settled. A loaded unit that is `inactive` is settled
/// too: it holds no failure and the manager may unload it whenever it likes,
/// so waiting for the unload itself would be waiting on garbage collection.
fn wait_until_settled(unit: &str) -> Result<()> {
    let started = Instant::now();
    loop {
        if let Some(reason) = unsettled_reason(scope_snapshot(unit)?.as_ref()) {
            ensure!(
                started.elapsed() < SETTLE_CEILING,
                "{unit}.scope did not settle within {SETTLE_CEILING:?} after teardown: {reason}"
            );
            thread::sleep(POLL_INTERVAL);
            continue;
        }
        return Ok(());
    }
}

/// Why a snapshot is not settled, or `None` when there is nothing left to do.
///
/// An unloaded unit and an `inactive` one both report settled, so the arms that
/// answer `None` are merged rather than left to look like distinct cases.
fn unsettled_reason(snapshot: Option<&ScopeSnapshot>) -> Option<String> {
    let state = snapshot?;
    if state.active == "failed" {
        Some(format!("it is still failed with Result={}", state.result))
    } else if is_busy(&state.active) {
        Some(format!("it is still {}", state.active))
    } else {
        None
    }
}
