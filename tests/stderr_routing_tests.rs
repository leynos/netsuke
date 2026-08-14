//! Request-level stream-routing tests with deliberately mismatched requests.
//!
//! The process layer must honour the request's explicit `stderr_mode` field
//! rather than whatever `cli.json` happens to say. The routing happens on real
//! child streams, so the worker runs in a dedicated subprocess whose stdout
//! and stderr the parent captures: the worker builds a request whose
//! `cli.json` contradicts its `stderr_mode`, runs a marker-emitting fake
//! Ninja, and the parent asserts where the markers landed.

use anyhow::{Context, Result, bail, ensure};
use mockable::{DefaultEnv, Env};
use netsuke::cli::Cli;
use netsuke::runner::{
    BuildTargets, CommandEnv, NinjaBuildRequest, NinjaToolRequest, StderrMode, run_ninja_tool_with,
    run_ninja_with,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{TempDir, tempdir};
use test_support::exec::write_exec_with_content;

const NINJA_ENV: &str = "NETSUKE_TEST_ROUTING_NINJA";
const JOB_ENV: &str = "NETSUKE_TEST_ROUTING_JOB";
const TOOL_ENV: &str = "NETSUKE_TEST_ROUTING_TOOL";
const RAN_FILE_ENV: &str = "NETSUKE_TEST_ROUTING_RAN_FILE";
const WORKER_NAME: &str = "routing_worker";

const STDOUT_MARKER: &str = "NETSUKE_ROUTING_STDOUT_MARKER";
const STDERR_MARKER: &str = "NETSUKE_ROUTING_STDERR_MARKER";

/// Write a fake Ninja that emits both stream markers and, separately, records
/// a run-marker file so the parent can tell whether the child ran at all.
///
/// The run-marker file is written directly by the child, independent of any
/// stream routing; it proves a suppression assertion observed output
/// *because the child ran*, not because it never spawned.
#[cfg(unix)]
fn marker_emitting_ninja(ran_file: &Path) -> Result<(TempDir, PathBuf)> {
    let dir = tempdir().context("create fake-ninja directory")?;
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' '{STDOUT_MARKER}'\nprintf '%s\\n' '{STDERR_MARKER}' >&2\n\
         touch '{}'\nexit 0\n",
        ran_file.to_string_lossy()
    );
    let path = write_exec_with_content(dir.path(), "fake-ninja", &script)
        .context("write fake ninja executable")?;
    Ok((dir, path))
}

/// Spawn the routing worker (this test binary) and capture its output.
#[cfg(unix)]
fn run_routing_worker(job: &str, tool: bool, ninja: &Path, ran_file: &Path) -> Result<Command> {
    let mut command = Command::new(std::env::current_exe().context("locate test binary")?);
    command
        .args(["--ignored", "--exact", WORKER_NAME, "--nocapture"])
        .env(NINJA_ENV, ninja)
        .env(JOB_ENV, job)
        .env(RAN_FILE_ENV, ran_file);
    if tool {
        command.env(TOOL_ENV, "1");
    }
    Ok(command)
}

/// A request whose `cli.json` and `stderr_mode` deliberately disagree: the
/// process layer must route streams by the explicit policy, not by `cli.json`.
#[cfg(unix)]
#[test]
#[ignore = "invoked as a stream-routing worker"]
fn routing_worker() -> Result<()> {
    let process_env = DefaultEnv;
    let job = process_env.raw(JOB_ENV).context("read routing job")?;
    let tool = process_env.os_string(TOOL_ENV).is_some();
    let ninja = PathBuf::from(
        process_env
            .os_string(NINJA_ENV)
            .context("read fake ninja path")?,
    );
    let stderr_mode = match job.as_str() {
        "forward" => StderrMode::Forward,
        "suppress" => StderrMode::Suppress,
        other => bail!("unknown routing job {other:?}"),
    };
    // The JSON flag contradicts `stderr_mode` so routing can only come from
    // the explicit policy field, not from the CLI JSON setting.
    let cli = Cli {
        json: !stderr_mode.is_suppress(),
        ..Cli::default()
    };
    let targets = BuildTargets::default();
    let env = CommandEnv::inherit();
    let result = if tool {
        run_ninja_tool_with(&NinjaToolRequest {
            program: &ninja,
            cli: &cli,
            build_file: Path::new("build.ninja"),
            tool: "clean",
            env: &env,
            stderr_mode,
        })
    } else {
        run_ninja_with(&NinjaBuildRequest {
            program: &ninja,
            cli: &cli,
            build_file: Path::new("build.ninja"),
            targets: &targets,
            env: &env,
            stderr_mode,
        })
    };
    result.context("run Ninja invocation in routing worker")
}

/// cli.json=true with `stderr_mode=Forward`: the child markers must reach the
/// user's stdout and stderr, proving routing ignores the JSON setting.
#[cfg(unix)]
#[test]
fn forward_request_routes_child_streams_despite_json_cli() -> Result<()> {
    let ran_dir = tempdir().context("create run-marker directory")?;
    let ran_file = ran_dir.path().join("ran");
    let (_ninja_dir, ninja) = marker_emitting_ninja(&ran_file)?;
    let output = run_routing_worker("forward", false, &ninja, &ran_file)?
        .output()
        .context("run forwarding build worker")?;
    ensure!(
        output.status.success(),
        "forwarding build worker failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8(output.stdout).context("worker stdout should be UTF-8")?;
    let stderr = String::from_utf8(output.stderr).context("worker stderr should be UTF-8")?;
    ensure!(
        stdout.contains(STDOUT_MARKER),
        "cli.json=true with StderrMode::Forward should forward child stdout, got: {stdout}"
    );
    ensure!(
        stderr.contains(STDERR_MARKER),
        "cli.json=true with StderrMode::Forward should forward child stderr, got: {stderr}"
    );
    Ok(())
}

/// cli.json=false with `stderr_mode=Suppress`: both child markers must be
/// drained even though the human CLI would otherwise forward them, and the
/// run marker proves the child really executed.
#[cfg(unix)]
#[test]
fn suppress_request_drains_child_streams_despite_human_cli() -> Result<()> {
    let ran_dir = tempdir().context("create run-marker directory")?;
    let ran_file = ran_dir.path().join("ran");
    let (_ninja_dir, ninja) = marker_emitting_ninja(&ran_file)?;
    let output = run_routing_worker("suppress", false, &ninja, &ran_file)?
        .output()
        .context("run suppressing build worker")?;
    ensure!(
        output.status.success(),
        "suppressing build worker failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    ensure!(ran_file.exists(), "fake Ninja should have run");
    let stdout = String::from_utf8(output.stdout).context("worker stdout should be UTF-8")?;
    let stderr = String::from_utf8(output.stderr).context("worker stderr should be UTF-8")?;
    ensure!(
        !stdout.contains(STDOUT_MARKER) && !stdout.contains(STDERR_MARKER),
        "cli.json=false with StderrMode::Suppress should drain child stdout, got: {stdout}"
    );
    ensure!(
        !stderr.contains(STDOUT_MARKER) && !stderr.contains(STDERR_MARKER),
        "cli.json=false with StderrMode::Suppress should drain child stderr, got: {stderr}"
    );
    Ok(())
}

/// cli.json=true with `stderr_mode=Forward` on the tool request: the child
/// markers must reach the user, proving the tool path also ignores cli.json.
#[cfg(unix)]
#[test]
fn forward_tool_request_routes_child_streams_despite_json_cli() -> Result<()> {
    let ran_dir = tempdir().context("create run-marker directory")?;
    let ran_file = ran_dir.path().join("ran");
    let (_ninja_dir, ninja) = marker_emitting_ninja(&ran_file)?;
    let output = run_routing_worker("forward", true, &ninja, &ran_file)?
        .output()
        .context("run forwarding tool worker")?;
    ensure!(
        output.status.success(),
        "forwarding tool worker failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8(output.stdout).context("worker stdout should be UTF-8")?;
    let stderr = String::from_utf8(output.stderr).context("worker stderr should be UTF-8")?;
    ensure!(
        stdout.contains(STDOUT_MARKER),
        "tool request with StderrMode::Forward should forward child stdout, got: {stdout}"
    );
    ensure!(
        stderr.contains(STDERR_MARKER),
        "tool request with StderrMode::Forward should forward child stderr, got: {stderr}"
    );
    Ok(())
}

/// cli.json=false with `stderr_mode=Suppress` on the tool request: both child
/// markers must be drained, with the run marker proving the child executed.
#[cfg(unix)]
#[test]
fn suppress_tool_request_drains_child_streams_despite_human_cli() -> Result<()> {
    let ran_dir = tempdir().context("create run-marker directory")?;
    let ran_file = ran_dir.path().join("ran");
    let (_ninja_dir, ninja) = marker_emitting_ninja(&ran_file)?;
    let output = run_routing_worker("suppress", true, &ninja, &ran_file)?
        .output()
        .context("run suppressing tool worker")?;
    ensure!(
        output.status.success(),
        "suppressing tool worker failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    ensure!(ran_file.exists(), "fake Ninja should have run");
    let stdout = String::from_utf8(output.stdout).context("worker stdout should be UTF-8")?;
    let stderr = String::from_utf8(output.stderr).context("worker stderr should be UTF-8")?;
    ensure!(
        !stdout.contains(STDOUT_MARKER) && !stdout.contains(STDERR_MARKER),
        "tool request with StderrMode::Suppress should drain child stdout, got: {stdout}"
    );
    ensure!(
        !stderr.contains(STDOUT_MARKER) && !stderr.contains(STDERR_MARKER),
        "tool request with StderrMode::Suppress should drain child stderr, got: {stderr}"
    );
    Ok(())
}
