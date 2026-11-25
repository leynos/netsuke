//! What the host must offer before a scenario can exercise the wrapper.
//!
//! A host missing a prerequisite cannot run the wrapper at all, so the honest
//! report is a skip naming what is missing — never a pass. The dedicated CI
//! lane sets `NETSUKE_KANI_SCOPE_WRAPPER_STRICT=1`, which turns that skip into
//! a failure: a lane that skipped every scenario would otherwise read as green
//! coverage while proving nothing.

use std::process::{Command, Output};

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};

/// The variable the dedicated CI lane sets to forbid skips.
const STRICT_VARIABLE: &str = "NETSUKE_KANI_SCOPE_WRAPPER_STRICT";

/// Decide whether the scenario can run, skipping or failing when it cannot.
///
/// Returns `Ok(true)` when every prerequisite is present. `Ok(false)` means
/// the scenario was reported as skipped and the caller must return without
/// asserting anything further; under strict mode the same condition is an
/// error rather than a skip.
pub(super) fn prerequisites_met(scenario: &str) -> Result<bool> {
    missing_prerequisite()?.map_or_else(
        || Ok(true),
        |reason| skip(scenario, &reason).map(|()| false),
    )
}

/// Check the host prerequisites and report what is missing.
///
/// Returns `Ok(None)` when the host can run the wrapper, or `Ok(Some(reason))`
/// naming the first unmet prerequisite. An absent `systemctl` and an absent
/// cgroup file are reported the same way: both are "this host cannot run the
/// wrapper", which the caller turns into a visible skip, and into a failure
/// only under strict mode. Returning them as errors instead would fail every
/// scenario on such a host, so a lane that merely lacks systemd would read as
/// a broken wrapper.
fn missing_prerequisite() -> Result<Option<String>> {
    // The user manager is the prerequisite `--user --scope` actually needs.
    let Some(manager) = run_probe(["systemctl", "--user", "is-system-running"])? else {
        return Ok(Some("`systemctl` is not available on this host".to_owned()));
    };
    let state = String::from_utf8_lossy(&manager.stdout).trim().to_owned();
    // `degraded` and `running` both mean the manager is up and can accept a
    // scope; only `offline` (or a missing binary) means it is not. `degraded`
    // is expected here: it is what accumulated failed units produce, and it is
    // the state this suite is meant to stop contributing to.
    if !matches!(state.as_str(), "running" | "degraded" | "starting") {
        return Ok(Some(format!(
            "no running per-user systemd manager (systemctl --user reported `{state}`)"
        )));
    }

    let Some(version) = run_probe(["systemctl", "--version"])? else {
        return Ok(Some(
            "`systemctl` disappeared between two probes".to_owned(),
        ));
    };
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
    // it needs are enabled in *its own* cgroup. A cgroup v1 host fails here,
    // and so does one whose cgroup tree is not mounted where the unified
    // hierarchy puts it: both are "cannot run the wrapper", not "the suite is
    // broken".
    Ok(check_delegation())
}

/// The controllers the wrapper's resource properties need in the user manager.
///
/// `CPUQuota` needs `cpu`, `MemoryMax`/`MemorySwapMax` need `memory`, and
/// `TasksMax` needs `pids`. `IOWeight` is not listed because `io` is not
/// delegated to user managers on every supported host, and a scope that cannot
/// apply it still runs.
const REQUIRED_CONTROLLERS: [&str; 3] = ["cpu", "memory", "pids"];

/// Confirm the user manager's cgroup has the controllers the wrapper needs.
///
/// The hierarchy root's `cgroup.controllers` is *not* the question. That file
/// lists what the root offers; a controller reaches a subtree only when each
/// cgroup on the path enables it in `cgroup.subtree_control`. A host can list
/// every controller at the root while the user slice enables none — measured on
/// the reference host, where `user.slice` enables `cpu memory pids` — and a
/// check against the root would then pass while every scope launch failed.
fn check_delegation() -> Option<String> {
    let manager_cgroup = format!(
        "/sys/fs/cgroup/user.slice/user-{}.slice/user@{}.service",
        rustix_uid(),
        rustix_uid()
    );
    let Ok(cgroup) = Dir::open_ambient_dir("/sys/fs/cgroup", ambient_authority()) else {
        return Some(
            "/sys/fs/cgroup is not available, so cgroup delegation cannot be confirmed".to_owned(),
        );
    };
    let relative = manager_cgroup.trim_start_matches("/sys/fs/cgroup/");
    let Ok(available) = cgroup.read_to_string(format!("{relative}/cgroup.controllers")) else {
        return Some(format!(
            "{manager_cgroup}/cgroup.controllers is missing, so the user manager's own \
             cgroup cannot be inspected; a cgroup v1 host reports the same"
        ));
    };
    let missing = missing_controllers(&available, &REQUIRED_CONTROLLERS);
    if !missing.is_empty() {
        return Some(format!(
            "the user manager's cgroup at {manager_cgroup} does not list {missing:?} in \
             cgroup.controllers"
        ));
    }
    // The manager's cgroup must also *enable* what it lists, or the scope it
    // creates gets nothing.
    let Ok(enabled) = cgroup.read_to_string(format!("{relative}/cgroup.subtree_control")) else {
        return Some(format!(
            "{manager_cgroup}/cgroup.subtree_control is missing, so the controllers the \
             user manager enables for its children cannot be confirmed"
        ));
    };
    let not_enabled = missing_controllers(&enabled, &REQUIRED_CONTROLLERS);
    if not_enabled.is_empty() {
        return None;
    }
    Some(format!(
        "the user manager's cgroup at {manager_cgroup} does not enable {not_enabled:?} in \
         cgroup.subtree_control"
    ))
}

/// The subset of `required` that `listing` does not name.
///
/// `cgroup.controllers` holds bare names and `cgroup.subtree_control` holds
/// names prefixed with `+` or `-`, so the comparison is on words rather than
/// on raw substrings.
fn missing_controllers(listing: &str, required: &[&str]) -> Vec<String> {
    let present: Vec<&str> = listing
        .split_whitespace()
        .map(|word| word.trim_start_matches(['+', '-']))
        .collect();
    required
        .iter()
        .filter(|name| !present.contains(name))
        .map(|name| (*name).to_owned())
        .collect()
}

/// The caller's numeric user id, without a `libc` dependency in this crate.
fn rustix_uid() -> u32 {
    // `id -u` is the one value every host running this suite can answer, and
    // the suite already shells out to `systemctl` for everything else.
    std::process::Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|output| String::from_utf8_lossy(&output.stdout).trim().parse().ok())
        .unwrap_or(u32::MAX)
}

/// Run one probe, mapping "the program is not installed" to `None`.
///
/// A probe that exists but fails is still an error: that is a host whose
/// systemd is present and misbehaving, which the caller should see rather than
/// have folded into a skip.
fn run_probe<const N: usize>(argv: [&str; N]) -> Result<Option<Output>> {
    let (program, arguments) = argv
        .split_first()
        .context("a probe needs a program to run")?;
    match Command::new(program).args(arguments).output() {
        Ok(output) => Ok(Some(output)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("probe the host with `{program}`")),
    }
}

/// Whether an unprovable scenario should fail the run instead of skipping.
///
/// This is the single site that reads the environment, and it is confined to
/// the test harness: nothing the suite launches depends on an ambient
/// variable, so there is no production seam to inject here.
#[expect(
    clippy::disallowed_methods,
    reason = "the dedicated CI lane signals strict mode through the environment; this is the single site that reads it"
)]
fn strict_mode() -> bool {
    std::env::var_os(STRICT_VARIABLE).is_some_and(|value| value == "1")
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
pub(super) fn skip(scenario: &str, reason: &str) -> Result<()> {
    ensure!(
        !strict_mode(),
        "{scenario} could not be exercised: {reason} (strict mode requires the \
         wrapper to be tested for real, so this is a failure rather than a skip)"
    );
    eprintln!("skipped: {scenario} — {reason}");
    Ok(())
}
