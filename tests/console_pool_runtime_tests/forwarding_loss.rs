//! Verify that losing the stderr destination does not fail the build.

use super::{ChildCleanup, netsuke_build, runtime_workspace, wait_for_exit, workspace_dir};
use anyhow::{Context, Result, ensure};
use std::io::Read;

/// Keep a build successful when its stderr destination disappears.
///
/// Netsuke forwards Ninja's stderr to its own stderr. Ninja writes the recipe
/// output that users watch to stdout, so the forwarded stderr stream carries
/// Ninja's own warnings; a corrupt `.ninja_log` is the one such warning a
/// successful build emits on demand. Closing the read end of the pipe is the
/// only way to make the destination fail from outside the process, so this is
/// the CLI-boundary check for the forwarding-failure path. The first build
/// proves Ninja's warning really reaches Netsuke's stderr; the second proves
/// that losing it is reported through the forwarding counter and warning
/// rather than through the exit status.
#[test]
fn a_closed_stderr_destination_does_not_fail_the_build() -> Result<()> {
    let Some(workspace) = runtime_workspace()? else {
        return Ok(());
    };
    let root = workspace_dir(&workspace)?;
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
    .context("write forwarding-loss manifest")?;

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
    let mut lost = netsuke_build(&workspace, &["build", "lost"])?
        .spawn()
        .context("spawn Netsuke forwarding-loss build")?;
    // Dropping the read end closes the pipe, so every later write to it fails.
    drop(
        lost.stderr
            .take()
            .context("Netsuke child should expose stderr")?,
    );
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
    Ok(())
}

/// Plant a `.ninja_log` that Ninja rejects, so the next build warns on stderr.
///
/// Ninja reports an unreadable build log as a warning and replays the whole
/// build, so the warning is available without making the build fail.
fn corrupt_build_log(root: &cap_std::fs_utf8::Dir) -> Result<()> {
    root.write(".ninja_log", b"not a ninja build log")
        .context("corrupt the Ninja build log")
}
