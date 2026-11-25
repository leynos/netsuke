//! What the user manager recorded, read without disturbing it.
//!
//! This module decides *what a reading means*: whether a unit is busy, whether
//! it has settled, and when to stop waiting. The readings themselves, and the
//! rules that keep an unloaded unit's defaults from being mistaken for
//! evidence, live in [`super::properties`] and are re-exported here so a
//! caller has one path to the manager's state.
//!
//! Nothing here stops, resets or reconfigures a unit: teardown lives in
//! [`super::guard`] and always names the one unit it owns. The capture and
//! journal reads are the durable evidence that outlives the unit, which is why
//! they live with the state decisions rather than with the property queries.
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

use super::harness::CAPTURE_DIR;

pub(super) use super::properties::{scope_lifetime, scope_property, scope_snapshot};

/// How long to wait for a scope to leave its active states.
///
/// A generous ceiling: it only ever elapses when the wrapper is broken, and
/// the scenarios it guards are bounded by `RUNTIME_CAP + STOP_GRACE`.
pub(super) const TERMINAL_CEILING: Duration = Duration::from_secs(45);

/// The interval between property reads while polling.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

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
        // The snapshot is atomic and already requires `LoadState=loaded`. An
        // `ActiveState` read on its own would answer `inactive` for an unloaded
        // unit, so an unexpected collection would be reported as a clean stop
        // instead of as the lost evidence it is.
        let Some(snapshot) = scope_snapshot(unit)? else {
            anyhow::bail!(
                "{unit}.scope was unloaded before reaching a terminal state, so the \
                 timeout evidence this scenario retains is no longer readable"
            );
        };
        if matches!(snapshot.active.as_str(), "failed" | "inactive") {
            return scope_lifetime(unit);
        }
        ensure!(
            started.elapsed() < TERMINAL_CEILING,
            "{unit}.scope was still {} after {TERMINAL_CEILING:?}",
            snapshot.active
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

/// What a journal read produced.
///
/// The distinction matters because the two absences have different meanings. A
/// journal that was read and holds nothing is real evidence: the unit logged
/// nothing. A journal that could not be read is not evidence at all, and
/// treating it as "logged nothing" would turn an unreadable host into a
/// silently passing assertion.
#[derive(Debug)]
pub(super) enum Journal {
    /// The journal was read after a successful sync; the text is final.
    Read(String),
    /// The journal was read, but `--sync` was refused.
    ///
    /// The text is real, but journald has not promised it is *complete*: a
    /// final entry may still be in flight. A scenario that needs a specific
    /// line therefore re-reads rather than trusting the first answer.
    Unsynced(String),
    /// The journal could not be read, for the stated reason.
    Unavailable(String),
}

impl Journal {
    /// Whether this read produced no usable text.
    ///
    /// Covers both an unreadable journal and an empty one, which is what a
    /// scenario means when it allows the journal to be absent.
    pub(super) fn is_empty(&self) -> bool {
        self.text().trim().is_empty()
    }

    /// The journal text, or an empty string when it could not be read.
    pub(super) fn text(&self) -> &str {
        match self {
            Self::Read(text) | Self::Unsynced(text) => text,
            Self::Unavailable(_) => "",
        }
    }

    /// Whether journald has promised the extract is complete.
    ///
    /// False when the sync was refused, which is when the text may still grow.
    fn is_synced(&self) -> bool {
        matches!(self, Self::Read(_))
    }

    /// The reason the journal was unavailable, if it was.
    pub(super) fn unavailable_reason(&self) -> Option<&str> {
        match self {
            Self::Read(_) | Self::Unsynced(_) => None,
            Self::Unavailable(reason) => Some(reason),
        }
    }
}

/// How long to keep re-reading the journal for expected text after a failed sync.
///
/// Only used when `--sync` was refused, which removes journald's storage
/// guarantee: the final entries may then arrive after this process has already
/// asked for them. Bounded so a genuinely missing entry still fails.
const JOURNAL_SETTLE_CEILING: Duration = Duration::from_secs(5);

/// How long to wait between journal re-reads.
const JOURNAL_SETTLE_INTERVAL: Duration = Duration::from_millis(200);

/// Read a bounded extract of the unit's journal.
///
/// The journal is the only record that survives collection, which is why the
/// evidence harvest reads it even when the unit is gone.
///
/// A read is preceded by `journalctl --sync`, because `journalctl` only
/// guarantees that messages written before a sync are stored. Without that
/// barrier the extract can miss the stop entries the assertions look for.
///
/// A query that exits non-zero is reported as [`Journal::Unavailable`] rather
/// than raised: a host may run without a readable user journal while the scope
/// itself worked, and the scenarios decide what an absent journal means for
/// the claim they are making.
pub(super) fn journal_extract(unit: &str) -> Journal {
    let synced = Command::new("journalctl")
        .args(["--user", "--sync"])
        .output()
        .is_ok_and(|output| output.status.success());
    match read_journal(unit) {
        Ok(text) if synced => Journal::Read(text),
        Ok(text) => Journal::Unsynced(text),
        Err(reason) => Journal::Unavailable(reason),
    }
}

/// Read the unit's bounded journal extract until `required` is present.
///
/// This is what a scenario uses when it asserts on specific entries. A
/// successful `--sync` already guarantees the extract is complete, so the
/// first read is final. When the sync was refused — as the reference host
/// refuses it, under Varlink — the guarantee is gone and a final entry may
/// still be in flight, so the read is repeated to a bounded deadline.
///
/// The deadline bounds *waiting*, not the claim: an entry that never appears
/// is still a failure, reported by the caller's assertion against the text
/// this returns.
pub(super) fn journal_extract_settled(unit: &str, required: &[&str]) -> Journal {
    let first = journal_extract(unit);
    if first.is_synced() {
        return first;
    }
    let started = Instant::now();
    let mut latest = first;
    while started.elapsed() < JOURNAL_SETTLE_CEILING {
        if required.iter().all(|needle| latest.text().contains(needle)) {
            return latest;
        }
        thread::sleep(JOURNAL_SETTLE_INTERVAL);
        latest = journal_extract(unit);
    }
    latest
}

/// Read the unit's bounded journal extract once.
///
/// Returns the text, or the reason it could not be read.
fn read_journal(unit: &str) -> std::result::Result<String, String> {
    let output = Command::new("journalctl")
        .args([
            "--user",
            "-q",
            "-u",
            &format!("{unit}.scope"),
            "--no-pager",
            "-o",
            "cat",
            "-n",
            "40",
        ])
        .output();
    match output {
        Ok(result) if result.status.success() => {
            Ok(String::from_utf8_lossy(&result.stdout).into_owned())
        }
        Ok(result) => Err(format!(
            "journalctl exited {:?}: {}",
            result.status.code(),
            String::from_utf8_lossy(&result.stderr).trim()
        )),
        Err(err) => Err(format!("journalctl could not be run: {err}")),
    }
}

/// Assert one scenario's own unit is not left failed, naming what it recorded.
///
/// The unit is named exactly, not matched by a prefix. Every test in one binary
/// shares a process id, and `cargo nextest` runs each test in its own process
/// but a same-binary run under libtest does not: a prefix query would then see
/// a sibling scenario's retained failure and blame this one for it. Naming the
/// unit keeps the claim attached to the work that actually owns it.
///
/// `context` states what the scenario promised, so a failure reads as the claim
/// that was contradicted rather than as a bare state dump.
pub(super) fn ensure_own_unit_settled(unit: &str, context: &str) -> Result<()> {
    let Some(state) = scope_snapshot(unit)? else {
        return Ok(());
    };
    ensure!(
        state.active != "failed" && !is_busy(&state.active),
        "{context}, but {unit}.scope is still {} with Result={}",
        state.active,
        state.result
    );
    Ok(())
}
