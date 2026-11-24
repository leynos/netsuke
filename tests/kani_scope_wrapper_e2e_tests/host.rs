//! What the host must offer before a scenario can exercise the wrapper.
//!
//! A host missing a prerequisite cannot run the wrapper at all, so the honest
//! report is a skip naming what is missing — never a pass. The dedicated CI
//! lane sets `NETSUKE_KANI_SCOPE_WRAPPER_STRICT=1`, which turns that skip into
//! a failure: a lane that skipped every scenario would otherwise read as green
//! coverage while proving nothing.

use std::process::Command;

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
/// naming the first unmet prerequisite.
fn missing_prerequisite() -> Result<Option<String>> {
    // The user manager is the prerequisite `--user --scope` actually needs.
    let manager = Command::new("systemctl")
        .args(["--user", "is-system-running"])
        .output()
        .context("probe for a per-user systemd manager")?;
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
