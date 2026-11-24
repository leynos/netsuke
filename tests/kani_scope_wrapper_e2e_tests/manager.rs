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
    collections::HashMap,
    process::Command,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::harness::CAPTURE_DIR;

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

/// The three properties every state decision needs, in one query.
///
/// Asking for them together is what makes a snapshot atomic. Three separate
/// queries can straddle an unload: the unit answers `LoadState=loaded`, is
/// collected, and then answers `ActiveState` and `Result` with defaults — so
/// the snapshot reports evidence the manager no longer holds. One query is one
/// observation, so the properties always describe the same instant.
const SNAPSHOT_PROPERTIES: [&str; 3] = ["LoadState", "ActiveState", "Result"];

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

/// Read several properties in one query, keyed by property name.
///
/// The named form is used rather than `-P` because `-P` prints bare values
/// with no keys to attach them to.
fn scope_properties(unit: &str, properties: &[&str]) -> Result<HashMap<String, String>> {
    let mut command = Command::new("systemctl");
    command.args(["--user", "show", &format!("{unit}.scope")]);
    for property in properties {
        command.arg("-p").arg(property);
    }
    let output = command
        .output()
        .with_context(|| format!("read {} for {unit}.scope", properties.join(", ")))?;
    ensure!(
        output.status.success(),
        "systemctl show {unit}.scope failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(parse_properties(&String::from_utf8_lossy(&output.stdout)))
}

/// Split `Name=value` lines into a map.
///
/// Only the first `=` separates a name from its value, because a value may
/// itself contain one.
fn parse_properties(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, value)| (name.to_owned(), value.trim().to_owned()))
        .collect()
}

/// Read the scope's state, or `None` when the unit is no longer loaded.
///
/// `None` is the honest answer for a unit that has been unloaded: its `Result`
/// is unavailable, not `success`. An absent key is treated the same way as
/// `LoadState=not-found`, since a property the manager did not report is not
/// evidence either.
pub(super) fn scope_snapshot(unit: &str) -> Result<Option<ScopeSnapshot>> {
    let properties = scope_properties(unit, &SNAPSHOT_PROPERTIES)?;
    if properties.get("LoadState").map(String::as_str) != Some("loaded") {
        return Ok(None);
    }
    Ok(Some(ScopeSnapshot {
        active: required_property(&properties, unit, "ActiveState")?,
        result: required_property(&properties, unit, "Result")?,
    }))
}

/// Take one property a loaded unit owes, failing when it is missing.
///
/// A loaded unit always reports all three, so an absent one means the output
/// was not understood. Defaulting it would fabricate evidence, which is the
/// failure mode [`scope_snapshot`] exists to prevent.
fn required_property(
    properties: &HashMap<String, String>,
    unit: &str,
    name: &str,
) -> Result<String> {
    properties.get(name).cloned().with_context(|| {
        format!(
            "systemctl show {unit}.scope reported LoadState=loaded but no {name}; \
             the properties read were {properties:?}"
        )
    })
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
            return Ok(started.elapsed());
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
    /// The journal was read; the text is whatever it held, possibly empty.
    Read(String),
    /// The journal could not be read, for the stated reason.
    Unavailable(String),
}

impl Journal {
    /// Whether this read produced no usable text.
    ///
    /// Covers both an unreadable journal and an empty one, which is what a
    /// scenario means when it allows the journal to be absent.
    pub(super) fn is_empty(&self) -> bool {
        match self {
            Self::Read(text) => text.trim().is_empty(),
            Self::Unavailable(_) => true,
        }
    }

    /// The journal text, or an empty string when it could not be read.
    pub(super) fn text(&self) -> &str {
        match self {
            Self::Read(text) => text,
            Self::Unavailable(_) => "",
        }
    }

    /// The reason the journal was unavailable, if it was.
    pub(super) fn unavailable_reason(&self) -> Option<&str> {
        match self {
            Self::Read(_) => None,
            Self::Unavailable(reason) => Some(reason),
        }
    }
}

/// Read a bounded extract of the unit's journal.
///
/// The journal is the only record that survives collection, which is why the
/// evidence harvest reads it even when the unit is gone.
///
/// A read is preceded by `journalctl --sync` so the unit's last messages are
/// on disk before the single query. Without that barrier the extract can miss
/// the stop entries the assertions look for: `journalctl` only guarantees that
/// messages written before a sync are stored. The sync is best-effort — a host
/// may refuse it, as the reference host does for the user journal under
/// Varlink — so its failure is not the query's failure.
///
/// A query that exits non-zero is reported as [`Journal::Unavailable`] rather
/// than raised: a host may run without a readable user journal while the scope
/// itself worked, and the scenarios decide what an absent journal means for
/// the claim they are making.
pub(super) fn journal_extract(unit: &str) -> Journal {
    drop(
        Command::new("journalctl")
            .args(["--user", "--sync"])
            .output(),
    );
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
            Journal::Read(String::from_utf8_lossy(&result.stdout).into_owned())
        }
        Ok(result) => Journal::Unavailable(format!(
            "journalctl exited {:?}: {}",
            result.status.code(),
            String::from_utf8_lossy(&result.stderr).trim()
        )),
        Err(err) => Journal::Unavailable(format!("journalctl could not be run: {err}")),
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
