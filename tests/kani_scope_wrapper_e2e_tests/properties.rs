//! Reading systemd unit properties, and knowing when a reading is not evidence.
//!
//! This is the layer that talks to `systemctl show`. It exists on its own
//! because the trap it guards is a *reading* trap rather than a policy one:
//! querying a unit that does not exist is not an error, and systemd answers
//! with the defaults for the requested properties — `Result=success` among
//! them. Every read here therefore either proves the manager really holds the
//! value or says it does not.
//!
//! # One query, one observation
//!
//! [`scope_snapshot`] asks for all three state properties in a single query.
//! Three separate queries can straddle an unload: the unit answers
//! `LoadState=loaded`, is collected, and then answers `ActiveState` and
//! `Result` with defaults, so the snapshot would report evidence the manager no
//! longer holds. One query is one observation, so the values always describe
//! the same instant.
//!
//! # Monotonic timestamps
//!
//! [`scope_lifetime`] reads the manager's own activation and inactivation
//! stamps rather than timing the call, because `systemd-run --scope` blocks
//! only until the launcher dies — which for a piped payload happens at the
//! runtime deadline, leaving the stop grace to run after the caller returns.

use std::{collections::HashMap, process::Command, time::Duration};

use anyhow::{Context, Result, ensure};

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
pub(super) fn scope_properties(unit: &str, properties: &[&str]) -> Result<HashMap<String, String>> {
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
pub(super) fn required_property(
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

/// The scope's own lifetime, from the manager's activation timestamps.
///
/// This is not the time spent waiting. `systemd-run --scope` blocks until the
/// *launcher* dies, which for the piped payload happens at the runtime
/// deadline; the grace period then runs with the caller already returned. A
/// clock started after the caller returns would therefore measure the grace
/// alone — about 3s of a 5s life — and could not tell an additive grace from a
/// nested one, which is the property the timeout scenarios exist to prove.
///
/// The manager records both instants itself, so the interval is the scope's
/// real lifetime. `InactiveEnterTimestampMonotonic` is only set once the unit
/// reaches a terminal state, which is why the caller waits for one first.
pub(super) fn scope_lifetime(unit: &str) -> Result<Duration> {
    let properties = scope_properties(
        unit,
        &[
            "ActiveEnterTimestampMonotonic",
            "InactiveEnterTimestampMonotonic",
        ],
    )?;
    let activated = monotonic_micros(&properties, unit, "ActiveEnterTimestampMonotonic")?;
    let deactivated = monotonic_micros(&properties, unit, "InactiveEnterTimestampMonotonic")?;
    Ok(Duration::from_micros(deactivated.saturating_sub(activated)))
}

/// Read one monotonic timestamp property, in microseconds since boot.
///
/// systemd reports these in microseconds and writes `0` (or an empty value)
/// when the instant has not happened, which for the inactivation stamp means
/// the unit has not stopped. Both cases fail here rather than yielding a
/// lifetime of zero.
fn monotonic_micros(properties: &HashMap<String, String>, unit: &str, name: &str) -> Result<u64> {
    let raw = required_property(properties, unit, name)?;
    ensure!(
        !raw.is_empty() && raw != "0",
        "{unit}.scope reports {name}={raw:?}, so its lifetime is not yet known"
    );
    raw.parse::<u64>()
        .with_context(|| format!("{unit}.scope reports a non-numeric {name}: {raw:?}"))
}
