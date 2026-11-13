//! Verify how the CLI reports and survives a lost output destination.

use super::{ChildCleanup, netsuke_build, runtime_workspace, wait_for_exit, workspace_dir};
use anyhow::{Context, Result, ensure};
use std::io::{Read, pipe};
use std::os::fd::OwnedFd;
use std::process::Stdio;

/// Report a failed stdout destination on stderr, which is still readable.
///
/// This is the CLI-observable proof that the forwarding-failure path is
/// reached: the counter lives in the process, and a failed stderr destination
/// cannot report its own failure, so breaking stdout and reading the warning
/// from stderr is the only way to see the path from outside.
#[test]
fn a_failed_stdout_destination_is_reported_on_stderr() -> Result<()> {
    let Some(workspace) = runtime_workspace()? else {
        return Ok(());
    };
    let root = workspace_dir(&workspace)?;
    write_manifest(&root)?;

    let mut child = netsuke_build(&workspace, &["build", "forwarded"])?
        .spawn()
        .context("spawn Netsuke forwarding build")?;
    // Dropping the read end closes the pipe before Ninja runs, so the first
    // forwarded byte is rejected.
    drop(
        child
            .stdout
            .take()
            .context("Netsuke child should expose stdout")?,
    );
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .context("Netsuke child should expose stderr")?
        .read_to_string(&mut stderr)
        .context("read Netsuke stderr")?;
    let status = wait_for_exit(&mut child)?;
    ensure!(
        status.success(),
        "a lost stdout destination must not fail the build: {status}"
    );

    ensure!(
        stderr.contains("failed to forward child output"),
        "the forwarding failure should be reported on stderr: {stderr:?}"
    );
    ensure!(
        stderr.contains("stream=\"stdout\""),
        "the warning should name the lost stream: {stderr:?}"
    );
    Ok(())
}

/// Keep a build successful and still reported when its stderr destination
/// disappears.
///
/// Netsuke forwards Ninja's stderr to its own stderr. Ninja writes the recipe
/// output that users watch to stdout, so the forwarded stderr stream carries
/// Ninja's own warnings; a corrupt `.ninja_log` is the one such warning a
/// successful build emits on demand. The first build proves that warning
/// really reaches Netsuke's stderr. The second hands the child a destination
/// that is already closed and requires the build to succeed, the recipe to
/// run, and the loss to be reported on stdout — a channel the closed pipe
/// cannot carry, so the assertion fails unless the forwarded write really was
/// rejected.
#[test]
fn a_closed_stderr_destination_does_not_fail_the_build() -> Result<()> {
    let Some(workspace) = runtime_workspace()? else {
        return Ok(());
    };
    let root = workspace_dir(&workspace)?;
    write_manifest(&root)?;

    corrupt_build_log(&root)?;
    let mut forwarded = netsuke_build(&workspace, &["build", "forwarded"])?
        .spawn()
        .context("spawn Netsuke forwarding build")?;
    let mut forwarded_stderr = String::new();
    forwarded
        .stderr
        .take()
        .context("Netsuke child should expose stderr")?
        .read_to_string(&mut forwarded_stderr)
        .context("read Netsuke stderr")?;
    let forwarded_status = wait_for_exit(&mut forwarded)?;
    ensure!(
        forwarded_status.success(),
        "the forwarding build failed with {forwarded_status}: {forwarded_stderr}"
    );
    ensure!(
        forwarded_stderr.contains("ninja: warning"),
        "Ninja's warning should reach Netsuke's stderr: {forwarded_stderr:?}"
    );

    corrupt_build_log(&root)?;
    // Close the read end before the child starts, so its very first write to
    // stderr meets a closed pipe. Closing it after `spawn` leaves a window in
    // which the child can fill the pipe buffer successfully, which would make
    // the rejection depend on scheduling rather than on the closed
    // destination.
    let (stderr_reader, stderr_writer) = pipe().context("create a stderr pipe")?;
    drop(stderr_reader);
    let mut lost_command = netsuke_build(&workspace, &["build", "lost"])?;
    lost_command.stderr(Stdio::from(OwnedFd::from(stderr_writer)));
    let lost = lost_command
        .spawn()
        .context("spawn Netsuke forwarding-loss build")?;
    let mut cleanup = ChildCleanup::new(lost, root.try_clone()?, Vec::new());
    let mut stdout = String::new();
    cleanup
        .child
        .stdout
        .take()
        .context("Netsuke child should expose stdout")?
        .read_to_string(&mut stdout)
        .context("read Netsuke stdout")?;
    let status = wait_for_exit(&mut cleanup.child)?;
    ensure!(
        status.success(),
        "a lost stderr destination must not fail the build: {status}"
    );
    cleanup.mark_finished();
    ensure!(
        root.exists("lost"),
        "the recipe should still run to completion"
    );
    ensure!(
        stdout.contains("lost"),
        "Ninja's stdout should still reach the user: {stdout:?}"
    );
    // This line can only be produced by the fallback that runs after the
    // stderr forwarding thread has reported a rejected write, so it is
    // positive evidence that the loss happened rather than a coincidence of
    // the warning arriving before the pipe closed.
    ensure!(
        stdout.contains("reporting on stdout because the stderr destination rejected the output"),
        "the lost stderr destination should be reported on stdout: {stdout:?}"
    );
    Ok(())
}

/// Write a manifest with one target per forwarding scenario.
fn write_manifest(root: &cap_std::fs_utf8::Dir) -> Result<()> {
    root.write(
        "Netsukefile",
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: forwarded\n",
            "    command: 'touch forwarded'\n",
            "  - name: lost\n",
            "    command: 'touch lost'\n",
        ),
    )
    .context("write forwarding-loss manifest")
}

/// Plant a `.ninja_log` that Ninja rejects, so the next build warns on stderr.
///
/// Ninja reports an unreadable build log as a warning and replays the whole
/// build, so the warning is available without making the build fail.
fn corrupt_build_log(root: &cap_std::fs_utf8::Dir) -> Result<()> {
    root.write(".ninja_log", b"not a ninja build log")
        .context("corrupt the Ninja build log")
}
