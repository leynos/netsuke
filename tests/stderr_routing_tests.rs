//! Request-level stream-routing tests with explicit policies.
//!
//! The process layer routes the child's stdout and stderr by the request's
//! explicit `stderr_mode` field alone. Converting CLI state into a policy
//! (`StderrMode::from_json_enabled`) happens upstream of these request types
//! and never reaches this layer. The routing happens on real child streams, so
//! the worker runs in a dedicated subprocess whose stdout and stderr the parent
//! captures: the worker builds a request carrying a fixed `stderr_mode`, runs a
//! marker-emitting fake Ninja, and the parent asserts where the markers landed.
#![cfg(unix)]

#![cfg(unix)]

use anyhow::{Context, Result, bail, ensure};
use mockable::{DefaultEnv, Env};
use netsuke::runner::{
    BuildTargets, CommandEnv, NinjaBuildRequest, NinjaProcessOptions, NinjaToolRequest, StderrMode,
    run_ninja_tool_with, run_ninja_with,
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
/// The run-marker path travels in `NETSUKE_TEST_ROUTING_RAN_FILE` (inherited by
/// the child through [`CommandEnv::inherit`]), so no path is interpolated into
/// the shell script. The run-marker file is written directly by the child,
/// independent of any stream routing; it proves a suppression assertion
/// observed output *because the child ran*, not because it never spawned.
fn marker_emitting_ninja() -> Result<(TempDir, PathBuf)> {
    let dir = tempdir().context("create fake-ninja directory")?;
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' '{STDOUT_MARKER}'\nprintf '%s\\n' '{STDERR_MARKER}' >&2\n\
         touch \"${RAN_FILE_ENV}\"\nexit 0\n",
    );
    let path = write_exec_with_content(dir.path(), "fake-ninja", &script)
        .context("write fake ninja executable")?;
    Ok((dir, path))
}

/// Spawn the routing worker (this test binary) and capture its output.
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

/// Execute one request-level routing case selected by `stderr_mode` and the
/// request path (`tool` selects the `ninja -t` request).
///
/// The request carries the explicit policy, so the marker assertions prove the
/// process layer routes child streams by the request field and never consults
/// CLI state to derive it.
fn assert_routing_case(stderr_mode: StderrMode, tool: bool) -> Result<()> {
    let job = match stderr_mode {
        StderrMode::Forward => "forward",
        StderrMode::Suppress => "suppress",
    };
    let path_label = if tool {
        "tool request"
    } else {
        "build request"
    };

    let ran_dir = tempdir().context("create run-marker directory")?;
    let ran_file = ran_dir.path().join("ran");
    let (_ninja_dir, ninja) = marker_emitting_ninja()?;
    let output = run_routing_worker(job, tool, &ninja, &ran_file)?
        .output()
        .with_context(|| format!("run {stderr_mode:?} {path_label} worker"))?;
    ensure!(
        output.status.success(),
        "{stderr_mode:?} {path_label} worker failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8(output.stdout).context("worker stdout should be UTF-8")?;
    let stderr = String::from_utf8(output.stderr).context("worker stderr should be UTF-8")?;
    match stderr_mode {
        StderrMode::Forward => {
            ensure!(
                stdout.contains(STDOUT_MARKER),
                "{path_label} with StderrMode::Forward should forward child stdout, got: {stdout}"
            );
            ensure!(
                stderr.contains(STDERR_MARKER),
                "{path_label} with StderrMode::Forward should forward child stderr, got: {stderr}"
            );
        }
        StderrMode::Suppress => {
            ensure!(ran_file.exists(), "fake Ninja should have run");
            ensure!(
                !stdout.contains(STDOUT_MARKER) && !stdout.contains(STDERR_MARKER),
                "{path_label} with StderrMode::Suppress should drain child stdout, got: {stdout}"
            );
            ensure!(
                !stderr.contains(STDOUT_MARKER) && !stderr.contains(STDERR_MARKER),
                "{path_label} with StderrMode::Suppress should drain child stderr, got: {stderr}"
            );
        }
    }
    Ok(())
}

/// Worker body: build a request carrying the explicit `stderr_mode` selected
/// by the parent, run it against the fake Ninja, and let the parent assert the
/// stream routing from the captured child markers.
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
    let options = NinjaProcessOptions::default();
    let targets = BuildTargets::default();
    let env = CommandEnv::inherit();
    let result = if tool {
        run_ninja_tool_with(&NinjaToolRequest {
            program: &ninja,
            options: &options,
            build_file: Path::new("build.ninja"),
            tool: "clean",
            env: &env,
            stderr_mode,
        })
    } else {
        run_ninja_with(&NinjaBuildRequest {
            program: &ninja,
            options: &options,
            build_file: Path::new("build.ninja"),
            targets: &targets,
            env: &env,
            stderr_mode,
        })
    };
    result.context("run Ninja invocation in routing worker")
}

/// A build request carrying `stderr_mode=Forward`: the child markers must
/// reach the user's stdout and stderr.
#[test]
fn forward_request_routes_child_streams() -> Result<()> {
    assert_routing_case(StderrMode::Forward, false)
}

/// A tool request carrying `stderr_mode=Forward`: the child markers must
/// reach the user's stdout and stderr.
#[test]
fn forward_tool_request_routes_child_streams() -> Result<()> {
    assert_routing_case(StderrMode::Forward, true)
}

/// A build request carrying `stderr_mode=Suppress`: both child markers must be
/// drained, and the run marker proves the child really executed.
#[test]
fn suppress_request_drains_child_streams() -> Result<()> {
    assert_routing_case(StderrMode::Suppress, false)
}

/// A tool request carrying `stderr_mode=Suppress`: both child markers must be
/// drained, with the run marker proving the child executed.
#[test]
fn suppress_tool_request_drains_child_streams() -> Result<()> {
    assert_routing_case(StderrMode::Suppress, true)
}
