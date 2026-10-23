//! Parent test that runs the deterministic workers in fresh processes.

use super::{CHILD_DEADLINE, MODE_ENV};
use anyhow::{Context, Result, anyhow, ensure};
use std::{
    io::Write,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

const WORKER_TEST: &str = "runner::manifest_structure_telemetry_reproduction::interest_worker";

/// Require the unmitigated and anchored schedules to reproduce in child processes.
#[test]
fn stale_summary_interest_is_reproduced_in_a_fresh_process() -> Result<()> {
    let output = run_worker_child("unmitigated")?;
    require_worker_markers(
        &output,
        &[
            "ISSUE927_GATE_PAUSED=true",
            "ISSUE927_RELEASE_CAUSE=installed",
            "ISSUE927_SPAN_FIELDS=[",
            "ISSUE927_EVENT_SNAPSHOT=[]",
            "ISSUE927_ORACLE=zero-rejected",
        ],
    )?;

    let mitigated_output = run_worker_child("mitigated")?;
    require_worker_markers(
        &mitigated_output,
        &[
            "ISSUE927_GATE_PAUSED=true",
            "ISSUE927_RELEASE_CAUSE=deadline",
            "ISSUE927_SPAN_FIELDS=[",
            "ISSUE927_EVENT_SNAPSHOT=[",
            "ISSUE927_ORACLE=one-accepted",
        ],
    )?;
    Ok(())
}

/// Require every expected schedule marker and forward the child's evidence.
fn require_worker_markers(output: &ChildOutput, markers: &[&str]) -> Result<()> {
    std::io::stdout()
        .lock()
        .write_all(output.stdout.as_bytes())
        .context("forward the child's required phase markers")?;
    for marker in markers {
        ensure!(
            output.stdout.contains(marker),
            "child output is missing {marker:?}; stdout:\n{}\nstderr:\n{}",
            output.stdout,
            output.stderr
        );
    }
    Ok(())
}

/// Run the ignored child worker with its schedule mode supplied through env.
fn run_worker_child(mode: &str) -> Result<ChildOutput> {
    let executable = std::env::current_exe().context("resolve the current test executable")?;
    let mut child = Command::new(executable)
        .args(["--ignored", "--exact", WORKER_TEST, "--nocapture"])
        .env(MODE_ENV, mode)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn the isolated issue 927 worker")?;
    let deadline = Instant::now() + CHILD_DEADLINE;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child
                    .wait_with_output()
                    .context("collect the completed issue 927 worker output")?;
                ensure!(
                    status.success(),
                    "issue 927 child exited with {status}; stdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                return Ok(ChildOutput::from(output));
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                return terminate_child(
                    child,
                    &format!("issue 927 child exceeded {CHILD_DEADLINE:?}"),
                );
            }
            Err(error) => {
                return terminate_child(
                    child,
                    &format!("could not inspect issue 927 child status: {error}"),
                );
            }
        }
    }
}

/// Stop the child and include its output in the fixture error.
fn terminate_child(mut child: Child, reason: &str) -> Result<ChildOutput> {
    let termination = child.kill().map_or_else(
        |error| format!("child termination failed: {error}"),
        |()| "child terminated".to_owned(),
    );
    let output = child
        .wait_with_output()
        .context("collect output after terminating the issue 927 worker")?;
    Err(anyhow!(
        "{reason}; {termination}; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

/// Own the captured output from the isolated worker process.
struct ChildOutput {
    stdout: String,
    stderr: String,
}

impl From<Output> for ChildOutput {
    /// Convert lossy OS output into diagnostic strings.
    fn from(output: Output) -> Self {
        Self {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}
