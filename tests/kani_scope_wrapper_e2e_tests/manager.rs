//! What the user manager recorded, read without disturbing it.
//!
//! Every read here is either a property query, a capture read, or a bounded
//! journal extract. Nothing stops, resets or reconfigures a unit: teardown
//! lives in [`super::guard`] and always names the one unit it owns.
//!
//! # Why a snapshot is an `Option`
//!
//! Querying a unit that does not exist is not an error. systemd answers with
//! the defaults for the requested properties, `Result=success` among them, and
//! `LoadState=not-found` is the only tell. [`scope_snapshot`] therefore
//! requires `LoadState=loaded` before it reports a `Result` at all, so a
//! collected unit yields `None` rather than fabricated evidence.

use std::{
    process::Command,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::harness::{CAPTURE_DIR, UNIT_PREFIX};

/// How long to wait for a scope to leave its active states.
///
/// A generous ceiling: it only ever elapses when the wrapper is broken, and
/// the scenarios it guards are bounded by `RUNTIME_CAP + STOP_GRACE`.
pub(super) const TERMINAL_CEILING: Duration = Duration::from_secs(45);

/// The interval between property reads while polling.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// A scope's manager-recorded state, read only while the unit is loaded.
///
/// Constructing one requires `LoadState=loaded`; an unloaded unit answers
/// every property query with a default, so a `Result` read from one is
/// fabricated rather than observed.
#[derive(Debug)]
pub(super) struct ScopeSnapshot {
    /// The scope's `ActiveState`.
    pub(super) active: String,
    /// The scope's `Result`, meaningful only because the unit is loaded.
    pub(super) result: String,
}

/// Read one systemd property for a scope.
///
/// The unit name is passed with its `.scope` suffix because a bare name is
/// resolved as a service and would never match.
pub(super) fn scope_property(unit: &str, property: &str) -> Result<String> {
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

/// Read the scope's state, or `None` when the unit is no longer loaded.
///
/// `None` is the honest answer for a unit that has been unloaded: its `Result`
/// is unavailable, not `success`.
pub(super) fn scope_snapshot(unit: &str) -> Result<Option<ScopeSnapshot>> {
    let load = scope_property(unit, "LoadState")?;
    if load != "loaded" {
        return Ok(None);
    }
    Ok(Some(ScopeSnapshot {
        active: scope_property(unit, "ActiveState")?,
        result: scope_property(unit, "Result")?,
    }))
}

/// Whether an `ActiveState` means the unit has not finished yet.
///
/// `deactivating` is the one that matters: the stop job is still running and
/// the grace period has not yet elapsed, so treating it as terminal would
/// measure the deadline and call it the bound.
pub(super) fn is_busy(active: &str) -> bool {
    matches!(
        active,
        "active" | "activating" | "deactivating" | "reloading" | "refreshing"
    )
}

/// Poll a scope until it is no longer active, returning its lifetime.
///
/// Only `failed` and `inactive` end the wait. A unit that unloads while still
/// busy is an error rather than a terminal state: the scenarios that call this
/// retain their unit precisely so its `Result` can be read, so an unloaded
/// unit means the evidence the caller needs is gone.
pub(super) fn wait_for_terminal(unit: &str) -> Result<Duration> {
    let started = Instant::now();
    loop {
        let state = scope_property(unit, "ActiveState")?;
        if matches!(state.as_str(), "failed" | "inactive") {
            return Ok(started.elapsed());
        }
        ensure!(
            scope_property(unit, "LoadState")? == "loaded",
            "{unit}.scope was unloaded while still {state}, so the timeout \
             evidence this scenario retains is no longer readable"
        );
        ensure!(
            started.elapsed() < TERMINAL_CEILING,
            "{unit}.scope was still {state} after {TERMINAL_CEILING:?}"
        );
        thread::sleep(POLL_INTERVAL);
    }
}

/// Poll until the unit is no longer loaded.
///
/// A unit unloads asynchronously after it stops, so "not found" is the only
/// answer that proves collection happened; a single read taken the moment the
/// caller returned can legitimately still see `loaded` and `inactive`.
pub(super) fn wait_until_unloaded(unit: &str) -> Result<()> {
    let started = Instant::now();
    loop {
        let load = scope_property(unit, "LoadState")?;
        if load != "loaded" {
            return Ok(());
        }
        ensure!(
            started.elapsed() < TERMINAL_CEILING,
            "{unit}.scope was still loaded {TERMINAL_CEILING:?} after its caller returned"
        );
        thread::sleep(POLL_INTERVAL);
    }
}

/// Poll until the unit is active, so a scenario can act on a running scope.
pub(super) fn wait_until_active(unit: &str) -> Result<()> {
    let started = Instant::now();
    loop {
        let state = scope_property(unit, "ActiveState")?;
        if state == "active" {
            return Ok(());
        }
        ensure!(
            started.elapsed() < TERMINAL_CEILING,
            "{unit}.scope never became active (last state {state:?})"
        );
        thread::sleep(POLL_INTERVAL);
    }
}

/// Open `/tmp` as a capability, which is the whole surface a capture needs.
fn capture_dir() -> Result<Dir> {
    Dir::open_ambient_dir(CAPTURE_DIR, ambient_authority())
        .with_context(|| format!("open {CAPTURE_DIR} to reach the captured output"))
}

/// Read the payload's captured output for `unit`.
///
/// The path is derived from the unit, so the read cannot disagree with the
/// write.
pub(super) fn captured(unit: &str) -> Result<String> {
    let capture = format!("{unit}.out");
    capture_dir()?
        .read_to_string(&capture)
        .with_context(|| format!("read the captured output from {CAPTURE_DIR}/{capture}"))
}

/// Whether the payload's captured output exists for `unit`.
pub(super) fn capture_exists(unit: &str) -> Result<bool> {
    let capture = format!("{unit}.out");
    capture_dir()?
        .try_exists(&capture)
        .with_context(|| format!("probe for the captured output at {CAPTURE_DIR}/{capture}"))
}

/// Remove the payload's captured output for `unit`.
///
/// A capture that was never written is already in the state this asks for, so
/// its absence is not an error; every other failure is.
pub(super) fn remove_capture(unit: &str) -> Result<()> {
    let capture = format!("{unit}.out");
    match capture_dir()?.remove_file(&capture) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err)
            .with_context(|| format!("remove the captured output at {CAPTURE_DIR}/{capture}")),
    }
}

/// Read a bounded extract of the unit's journal.
///
/// The journal is the only record that survives collection, which is why the
/// evidence harvest reads it even when the unit is gone.
pub(super) fn journal_extract(unit: &str) -> Result<String> {
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
        .with_context(|| format!("read the journal for {unit}.scope"))?;
    ensure!(
        output.status.success(),
        "journalctl for {unit}.scope failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Assert that no unit this process owns is left failed, naming what is.
///
/// `context` states the claim the answer contradicts, so a failure reads as a
/// broken promise rather than as a bare list.
pub(super) fn ensure_no_failed_units(context: &str) -> Result<()> {
    let failed = own_failed_units()?;
    ensure!(
        failed.is_empty(),
        "{context}, but these units this process owns are still failed: {failed:?}"
    );
    Ok(())
}

/// The failed units this process owns, read without touching any of them.
///
/// The glob carries the process id, so a concurrent run in another test
/// process cannot contribute to or be blamed for this answer. Nothing here
/// stops or resets anything: it is a query, and a glob is safe to query with.
pub(super) fn own_failed_units() -> Result<Vec<String>> {
    let pattern = format!("{UNIT_PREFIX}-{}-*", std::process::id());
    let output = Command::new("systemctl")
        .args([
            "--user",
            "list-units",
            "--all",
            "--state=failed",
            "--no-legend",
            "--plain",
            "--no-pager",
            &pattern,
        ])
        .output()
        .context("list this process's failed scopes")?;
    ensure!(
        output.status.success(),
        "systemctl --user list-units --state=failed `{pattern}` failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect())
}
