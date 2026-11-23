//! The durable record of one finished invocation, harvested after the caller returns.
//!
//! A scope's `Result` is assigned by the manager, not copied from the payload,
//! and the two can disagree in both directions. A payload that exits 7 leaves
//! a scope the manager collects as a success, because a scope has no main
//! process and nothing maps an exit status onto `Result`; a payload that exits
//! zero can leave a scope recorded as `Result=timeout`, because the runtime
//! deadline covers the whole control group rather than the payload alone.
//! Neither figure is a substitute for the other, so an [`Outcome`] records
//! both and says which is which.
//!
//! What survives the unit is the caller's exit status, the captured output,
//! and the journal. What does not survive is everything the manager holds in
//! memory: once a unit is unloaded its execution results are gone. The harvest
//! therefore reports `Result` only from a loaded unit and says "unavailable"
//! otherwise, rather than printing the default systemd answers with.

use std::process::Output;

use anyhow::{Result, ensure};

use super::{
    harness::capture_path,
    manager::{ScopeSnapshot, captured, is_busy, journal_extract, scope_snapshot},
};

/// Everything one finished invocation left behind.
pub(super) struct Outcome {
    /// The unit name, without the `.scope` suffix.
    pub(super) unit: String,
    /// Whether `systemd-run` itself reported success.
    pub(super) caller_success: bool,
    /// The caller's exit code, or `None` when it died from a signal.
    pub(super) caller_code: Option<i32>,
    /// Where the payload's output was captured, whether or not it was written.
    pub(super) capture_path: String,
    /// The captured output, read after the caller returned.
    pub(super) capture: String,
    /// A bounded extract of the unit's journal.
    pub(super) journal: String,
    /// The manager's state, or `None` when the unit is already unloaded.
    snapshot: Option<ScopeSnapshot>,
}

impl Outcome {
    /// Harvest the caller status, the capture, the unit identity, and a journal extract.
    ///
    /// The snapshot is taken last so it reflects the state after the capture
    /// and journal reads. Every one of those reads is independent of the unit
    /// staying loaded, which is what makes the record usable for a collected
    /// scope as well as a retained one.
    pub(super) fn harvest(unit: &str, caller: &Output) -> Result<Self> {
        Ok(Self {
            unit: unit.to_owned(),
            caller_success: caller.status.success(),
            caller_code: caller.status.code(),
            capture_path: capture_path(unit),
            capture: captured(unit)?,
            journal: journal_extract(unit)?,
            snapshot: scope_snapshot(unit)?,
        })
    }

    /// The caller's exit status, naming a signal death rather than omitting it.
    pub(super) fn caller_description(&self) -> String {
        describe_caller(self.caller_code)
    }

    /// The `ActiveState` and `Result` of a unit that is still loaded.
    ///
    /// Fails when the unit has been unloaded, because the only values left to
    /// read at that point are the defaults systemd answers with. Returning
    /// them would turn "the evidence is gone" into a fabricated `success`.
    pub(super) fn loaded_state(&self) -> Result<(&str, &str)> {
        match &self.snapshot {
            Some(snapshot) => Ok((snapshot.active.as_str(), snapshot.result.as_str())),
            None => anyhow::bail!(
                "{}.scope is no longer loaded, so its Result cannot be read; an \
                 unloaded unit answers property queries with defaults: {}",
                self.unit,
                self.describe()
            ),
        }
    }

    /// Fail unless the manager left nothing failed or running behind.
    ///
    /// `expectation` states what the scenario claimed, so the diagnostic reads
    /// as the claim that was contradicted rather than as a bare state dump.
    pub(super) fn ensure_no_failed_scope(&self, expectation: &str) -> Result<()> {
        let Some(snapshot) = &self.snapshot else {
            return Ok(());
        };
        ensure!(
            state_is_clean(snapshot),
            "{expectation}, but the manager left {}.scope {}: {}",
            self.unit,
            describe_state(snapshot),
            self.describe()
        );
        Ok(())
    }

    /// A multi-line report of every figure this harvest observed.
    ///
    /// Used as assertion context, so it names the unit, the caller status, the
    /// capture, the manager state, and the journal — the four things a reader
    /// needs to tell a deadline from an OOM kill from an external stop.
    pub(super) fn describe(&self) -> String {
        format!(
            "unit={}.scope caller={} capture={} ({} bytes) manager={} journal:\n{}",
            self.unit,
            describe_caller(self.caller_code),
            self.capture_path,
            self.capture.len(),
            describe_snapshot(&self.snapshot),
            indent(&self.journal)
        )
    }
}

/// Whether a snapshot holds nothing a teardown would need to act on.
fn state_is_clean(snapshot: &ScopeSnapshot) -> bool {
    snapshot.active != "failed" && !is_busy(&snapshot.active)
}

/// Render a caller's exit status, naming a signal death rather than omitting it.
fn describe_caller(code: Option<i32>) -> String {
    match code {
        Some(value) => value.to_string(),
        None => "killed by a signal".to_owned(),
    }
}

/// Render one `ActiveState` and `Result` pair.
fn describe_state(snapshot: &ScopeSnapshot) -> String {
    format!("ActiveState={} Result={}", snapshot.active, snapshot.result)
}

/// Render the manager's state, saying plainly when there is nothing to render.
fn describe_snapshot(snapshot: &Option<ScopeSnapshot>) -> String {
    match snapshot {
        Some(state) => format!("LoadState=loaded {}", describe_state(state)),
        None => concat!(
            "LoadState=not-found (unloaded, so Result is unavailable; systemd ",
            "answers property queries for an absent unit with defaults, and any ",
            "Result read from one would be fabricated)"
        )
        .to_owned(),
    }
}

/// Indent a journal extract so it reads as a block inside an assertion message.
fn indent(text: &str) -> String {
    if text.trim().is_empty() {
        return "    <the journal has no entries for this unit>".to_owned();
    }
    text.lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}
