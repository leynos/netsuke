//! The documented scope wrapper, driven at test-sized limits.
//!
//! Every scenario needs the same three things: a unit name no other run can
//! collide with, an invocation that matches the shape
//! `docs/developers-guide.md` documents, and a way to read what the manager
//! recorded. The invocation and its naming live here; reading the manager's
//! record lives in [`super::manager`], so a scenario states its own property
//! rather than restating the command line.
//!
//! # Naming
//!
//! A name carries the process id, a start stamp fixed once per test process,
//! and a process-wide counter. Nextest runs each test in its own process, so
//! the process id alone already separates concurrent tests; the stamp and the
//! counter also separate successive runs that happen to reuse a pid, which is
//! what left same-named units behind when cleanup was skipped. The stamp is
//! fixed at first use rather than read per call so a name is reproducible
//! within one run.
//!
//! # Two runtime caps
//!
//! [`RUNTIME_CAP`] is the cap the timeout scenarios assert against: it is
//! deliberately short so a stop is cheap to observe. [`ROUTINE_RUNTIME_CAP`]
//! is the one a scenario that expects its payload to finish uses. The
//! distinction is load-bearing. A scope's runtime deadline covers the whole
//! control group, not just the payload, so on a loaded host the deadline can
//! fire while the pipeline is still tearing down — after the payload has
//! already exited zero. A routine scenario that inherited the short cap would
//! then be stopped by its own bound and leave a `Result=timeout` unit behind.

use std::{
    process::{Child, Command, Output},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};

/// The runtime bound the timeout scenarios assert against.
///
/// Test-sized rather than the documented eight minutes: the wrapper's
/// behaviour depends on the ratio between this and [`STOP_GRACE`], not on
/// their magnitude.
pub(super) const RUNTIME_CAP: Duration = Duration::from_secs(2);

/// The runtime bound for scenarios whose payload is expected to finish.
///
/// Wide enough that only a genuinely wedged run reaches it, so a routine
/// scenario cannot be stopped by its own deadline on a loaded host and report
/// a failure the payload never caused. Bounded so a hang still ends, and kept
/// below [`super::manager::TERMINAL_CEILING`] minus [`STOP_GRACE`] so the wait
/// for a terminal state cannot outlast its own ceiling.
pub(super) const ROUTINE_RUNTIME_CAP: Duration = Duration::from_secs(30);

/// The stop grace period paired with [`RUNTIME_CAP`].
///
/// The wrapper's effective bound is the sum, so this is also what makes the
/// additive-grace assertion measurable.
pub(super) const STOP_GRACE: Duration = Duration::from_secs(3);

/// How long a scenario's payload would run if nothing stopped it.
///
/// Comfortably longer than `RUNTIME_CAP + STOP_GRACE`, so a payload that
/// finishes on its own can never be mistaken for one the scope stopped.
pub(super) const PAYLOAD_LIFETIME: Duration = Duration::from_secs(60);

/// Slack allowed above the nominal bound before a scenario is judged overrun.
///
/// A scope's stop is not instantaneous: the deadline expires, the stop job
/// runs, and a non-cooperative payload is killed once the grace period lapses.
/// Observed overshoot on the reference host was under a second.
pub(super) const TIMING_SLACK: Duration = Duration::from_secs(4);

/// The shared prefix of every unit this suite creates.
pub(super) const UNIT_PREFIX: &str = "netsuke-kani-wrapper";

/// The directory the payloads write their captures into.
pub(super) const CAPTURE_DIR: &str = "/tmp";

/// What the manager does with a scope once it reaches a terminal state.
///
/// `Collect` passes `--collect`, which is `CollectMode=inactive-or-failed`:
/// the unit is unloaded even when it failed. `Retain` leaves the default
/// `CollectMode=inactive`, so a failed unit stays loaded and its `Result`,
/// statistics and journal can still be read after the caller returns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Retention {
    /// Unload the unit once it finishes, whether it succeeded or failed.
    Collect,
    /// Keep a failed unit loaded so its evidence can be read.
    Retain,
}

/// A unique unit name for one scenario.
///
/// See the module documentation for what each component keeps apart.
pub(super) fn unit_name(scenario: &str) -> String {
    static START_STAMP: OnceLock<String> = OnceLock::new();
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    let stamp = START_STAMP.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis())
            .to_string()
    });
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!(
        "{UNIT_PREFIX}-{}-{stamp}-{sequence}-{scenario}",
        std::process::id()
    )
}

/// The capture file for one unit, as an absolute path.
///
/// Derived rather than passed. Both call sites of the capture path need it to
/// agree with the unit, and three adjacent `&str` parameters make a
/// transposed `unit`/`capture` pair a silent bug: the scope would write one
/// file and the assertion would read another, which fails confusingly or,
/// worse, passes.
pub(super) fn capture_path(unit: &str) -> String {
    format!("{CAPTURE_DIR}/{unit}.out")
}

/// Build a `systemd-run` command with a controlled environment.
///
/// The caller's environment is cleared, because inheriting it would let an
/// ambient variable change what these scenarios measure. `SHELLOPTS=pipefail`
/// is the concrete case: bash imports `SHELLOPTS` at startup, so an exported
/// value would enable `pipefail` in the control that exists to show what
/// happens *without* it, and that control would then report 7 instead of 0 —
/// measured on the reference host.
///
/// Only what reaching the user manager requires is restored. `XDG_RUNTIME_DIR`
/// is how `--user` finds the bus, and `DBUS_SESSION_BUS_ADDRESS` is honoured
/// when the host sets it.
#[expect(
    clippy::disallowed_methods,
    reason = "this is the seam itself: it reads the ambient environment once to build a child's controlled one, and nothing else in the suite reads an ambient variable"
)]
fn managed_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.env_clear();
    for name in ["XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS", "PATH"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
}

/// Build the wrapper exactly as the plans document it, with test-sized limits.
///
/// `payload` is the shell body placed inside the scope. The resource
/// properties, `nice` level, and capture placement mirror the documented
/// command; only the two time limits are reduced, and `--collect` is added
/// only for [`Retention::Collect`].
fn scope_command(unit: &str, cap: Duration, retention: Retention, payload: &str) -> Command {
    let log = capture_path(unit);
    // The payload is brace-grouped so that the pipeline's left side is the
    // whole group. Without the braces a trailing `exit` in the payload would
    // end the shell before `tee` ever ran, leaving an empty capture and a
    // pipeline status that says nothing about the payload.
    let body = format!("set -o pipefail; {{ {payload}; }} 2>&1 | tee {log}");
    let mut command = managed_command("systemd-run");
    command.args([
        "--user",
        "--scope",
        "--expand-environment=no",
        "--unit",
        unit,
    ]);
    if retention == Retention::Collect {
        command.arg("--collect");
    }
    command.args([
        "-p",
        &format!("RuntimeMaxSec={}s", cap.as_secs()),
        "-p",
        &format!("TimeoutStopSec={}s", STOP_GRACE.as_secs()),
        "-p",
        "CPUQuota=200%",
        "-p",
        "MemoryMax=8G",
        "-p",
        "MemorySwapMax=0",
        "-p",
        "TasksMax=96",
        "-p",
        "IOWeight=20",
        "/usr/bin/nice",
        "-n",
        "15",
        "bash",
        "-c",
        &body,
    ]);
    command
}

/// Run the wrapper to completion and return its output.
pub(super) fn run_scoped(
    unit: &str,
    cap: Duration,
    retention: Retention,
    payload: &str,
) -> Result<Output> {
    scope_command(unit, cap, retention, payload)
        .output()
        .context("run the documented systemd-run scope wrapper")
}

/// Spawn the wrapper without waiting, so a scenario can observe it running.
pub(super) fn spawn_scoped(
    unit: &str,
    cap: Duration,
    retention: Retention,
    payload: &str,
) -> Result<Child> {
    scope_command(unit, cap, retention, payload)
        .spawn()
        .context("spawn the documented systemd-run scope wrapper")
}

/// The control for the `pipefail` assertion: the same body without `pipefail`.
///
/// This is deliberately not [`run_scoped`]. The control exists to show what
/// the wrapper's `pipefail` is protecting against, so it must differ from the
/// documented shape in exactly that property — it also omits the resource
/// properties and `nice`, which is the shape the wrapper replaced.
///
/// The environment is controlled here too, and for this scenario it is
/// load-bearing: an inherited `SHELLOPTS=pipefail` would turn the control into
/// the thing it is meant to contrast with.
pub(super) fn run_without_pipefail(unit: &str, cap: Duration) -> Result<Output> {
    let log = capture_path(unit);
    let body = format!("{{ echo masked-failure; exit 7; }} 2>&1 | tee {log}");
    managed_command("systemd-run")
        .args([
            "--user",
            "--scope",
            "--expand-environment=no",
            "--unit",
            unit,
            "-p",
            &format!("RuntimeMaxSec={}s", cap.as_secs()),
            "-p",
            &format!("TimeoutStopSec={}s", STOP_GRACE.as_secs()),
            "bash",
            "-c",
            &body,
        ])
        .output()
        .context("run the wrapper without pipefail")
}
