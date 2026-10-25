//! Real-process checks for prompt console output and exclusive console edges.
//!
//! Each scenario launches Netsuke with piped output and controls recipe
//! completion through files, proving behaviour at the Ninja process boundary.

#![cfg(unix)]

use anyhow::{Context, Result, bail, ensure};
use assert_cmd::cargo::cargo_bin;
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use mockable::{DefaultEnv, Env};
use std::{
    ffi::OsString,
    io::{self, BufRead, BufReader, Read},
    process::{Child, ChildStdout, Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tempfile::TempDir;
use test_support::ninja::ninja_integration_workspace;

const PROCESS_DEADLINE: Duration = Duration::from_secs(60);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Release blocked recipes and reap Netsuke if a runtime assertion fails.
struct ChildCleanup {
    child: Child,
    workspace: Dir,
    release_files: Vec<Utf8PathBuf>,
    finished: bool,
}

impl ChildCleanup {
    /// Retain the child and the files that unblock its recipes.
    fn new(child: Child, workspace: Dir, release_files: Vec<Utf8PathBuf>) -> Self {
        Self {
            child,
            workspace,
            release_files,
            finished: false,
        }
    }

    /// Mark a successfully reaped child so cleanup does not send it a signal.
    fn mark_finished(&mut self) {
        self.finished = true;
    }
}

impl Drop for ChildCleanup {
    fn drop(&mut self) {
        for release_file in &self.release_files {
            drop(self.workspace.write(release_file, b""));
        }
        if !self.finished {
            let deadline = Instant::now() + Duration::from_secs(2);
            while matches!(self.child.try_wait(), Ok(None)) && Instant::now() < deadline {
                thread::sleep(POLL_INTERVAL);
            }
            if matches!(self.child.try_wait(), Ok(None)) {
                drop(self.child.kill());
            }
        }
        drop(self.child.wait());
    }
}

/// Create a Netsuke child with a private configuration environment and piped output.
///
/// # Errors
///
/// Returns an error when the host PATH is unavailable or the child configuration
/// directory cannot be created.
fn netsuke_build(workspace: &TempDir, arguments: &[&str]) -> Result<Command> {
    let path = host_path()?;
    let config_home = workspace.path().join(".config");
    workspace_dir(workspace)?
        .create_dir_all(".config")
        .context("create child configuration directory")?;
    let mut command = Command::new(cargo_bin!("netsuke"));
    command
        .current_dir(workspace.path())
        .env_clear()
        .env("PATH", path)
        .env("HOME", workspace.path())
        .env("XDG_CONFIG_HOME", config_home)
        .args(["--progress", "never"])
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(command)
}

/// Open a capability directory for a temporary process workspace.
///
/// # Errors
///
/// Returns an error when the temporary path is not UTF-8 or cannot be opened.
fn workspace_dir(workspace: &TempDir) -> Result<Dir> {
    let root = Utf8PathBuf::from_path_buf(workspace.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("temporary workspace path is not UTF-8: {path:?}"))?;
    Dir::open_ambient_dir(root, ambient_authority()).context("open temporary workspace")
}

/// Read the host `PATH` for the isolated Netsuke child process.
///
/// # Errors
///
/// Returns an error when the host environment does not provide `PATH`.
fn host_path() -> Result<OsString> {
    DefaultEnv
        .os_string("PATH")
        .context("host PATH is required to run Ninja")
}

/// Read stdout on a helper thread so the parent can observe lines before exit.
fn spawn_line_reader(stdout: ChildStdout) -> (Receiver<io::Result<String>>, JoinHandle<()>) {
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => drop(sender.send(Ok(line))),
                Err(error) => {
                    drop(sender.send(Err(error)));
                    break;
                }
            }
        }
    });
    (receiver, reader)
}

/// Return the next output line before the bounded deadline.
///
/// # Errors
///
/// Returns an error when stdout closes, fails to read, or misses the deadline.
fn receive_line(receiver: &Receiver<io::Result<String>>, deadline: Instant) -> Result<String> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    ensure!(!remaining.is_zero(), "timed out waiting for Netsuke output");
    match receiver.recv_timeout(remaining) {
        Ok(Ok(line)) => Ok(line),
        Ok(Err(error)) => Err(error).context("read Netsuke stdout"),
        Err(RecvTimeoutError::Timeout) => bail!("timed out waiting for Netsuke output"),
        Err(RecvTimeoutError::Disconnected) => {
            bail!("Netsuke stdout closed before the expected marker appeared")
        }
    }
}

/// Wait for a marker line while the recipe remains blocked on its release file.
///
/// # Errors
///
/// Returns an error when stdout closes or the marker does not arrive in time.
fn wait_for_marker(receiver: &Receiver<io::Result<String>>, marker: &str) -> Result<String> {
    let deadline = Instant::now() + PROCESS_DEADLINE;
    loop {
        let line = receive_line(receiver, deadline)?;
        if line.trim_start().starts_with(marker) {
            return Ok(line);
        }
    }
}

/// Wait for the Netsuke process to exit before the bounded deadline.
///
/// # Errors
///
/// Returns an error when process status cannot be read or the child misses the deadline.
fn wait_for_exit(child: &mut Child) -> Result<ExitStatus> {
    let deadline = Instant::now() + PROCESS_DEADLINE;
    loop {
        if let Some(status) = child.try_wait().context("poll Netsuke child status")? {
            return Ok(status);
        }
        ensure!(
            Instant::now() < deadline,
            "Netsuke build did not exit before the deadline"
        );
        thread::sleep(POLL_INTERVAL);
    }
}

/// Require a successful build and retain the bounded child diagnostic on failure.
///
/// # Errors
///
/// Returns an error when the child does not exit successfully or stderr cannot be read.
fn finish_successfully(cleanup: &mut ChildCleanup) -> Result<()> {
    let status = wait_for_exit(&mut cleanup.child)?;
    let mut stderr = String::new();
    cleanup
        .child
        .stderr
        .take()
        .context("Netsuke child should expose stderr")?
        .read_to_string(&mut stderr)
        .context("read Netsuke stderr")?;
    ensure!(
        status.success(),
        "Netsuke build failed with {status}: {stderr}"
    );
    cleanup.mark_finished();
    Ok(())
}

/// Probe Ninja or skip this optional local runtime test when it is unavailable.
fn runtime_workspace() -> Option<TempDir> {
    match ninja_integration_workspace() {
        Ok(workspace) => Some(workspace),
        Err(error) => {
            tracing::warn!(%error, "skipping console runtime test without Ninja");
            None
        }
    }
}

/// Forward a flushed console-pool marker before the blocked command completes.
#[test]
fn console_output_reaches_piped_stdout_before_the_recipe_exits() -> Result<()> {
    let Some(workspace) = runtime_workspace() else {
        return Ok(());
    };
    let root = workspace_dir(&workspace)?;
    let release_file = Utf8PathBuf::from(".release-stream");
    root.write(
        "Netsukefile",
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: streamed\n",
            "    command: 'printf \"STREAM-READY\\n\"; while [ ! -f .release-stream ]; do sleep 0.02; done; touch streamed'\n",
            "    console: true\n",
        ),
    )
    .context("write streaming console manifest")?;

    let child = netsuke_build(&workspace, &["build", "streamed"])?
        .spawn()
        .context("spawn Netsuke streaming build")?;
    let mut cleanup = ChildCleanup::new(child, root.try_clone()?, vec![release_file.clone()]);
    let stdout = cleanup
        .child
        .stdout
        .take()
        .context("Netsuke child should expose stdout")?;
    let (output, reader) = spawn_line_reader(stdout);

    wait_for_marker(&output, "STREAM-READY")?;
    ensure!(
        cleanup
            .child
            .try_wait()
            .context("check streaming child status")?
            .is_none(),
        "the recipe exited before the parent released it"
    );
    root.write(&release_file, b"release")
        .context("release streaming recipe")?;
    finish_successfully(&mut cleanup)?;
    reader
        .join()
        .map_err(|_| anyhow::anyhow!("stdout forwarding thread panicked"))?;
    ensure!(
        root.exists("streamed"),
        "the released recipe should create its output"
    );
    Ok(())
}

/// Keep two console recipes from overlapping under a parallel Ninja job limit.
#[test]
fn console_edges_are_exclusive_with_parallel_jobs_enabled() -> Result<()> {
    let Some(workspace) = runtime_workspace() else {
        return Ok(());
    };
    let root = workspace_dir(&workspace)?;
    let first_release = Utf8PathBuf::from(".release-one");
    let second_release = Utf8PathBuf::from(".release-two");
    root.write(
        "Netsukefile",
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: one\n",
            "    command: >-\n",
            "      mkdir .console-running 2>/dev/null || { echo COLLISION >&2; exit 1; };\n",
            "      printf 'START:one\\n';\n",
            "      while [ ! -f .release-one ]; do sleep 0.02; done;\n",
            "      rmdir .console-running; touch one; printf 'DONE:one\\n'\n",
            "    console: true\n",
            "  - name: two\n",
            "    command: >-\n",
            "      mkdir .console-running 2>/dev/null || { echo COLLISION >&2; exit 1; };\n",
            "      printf 'START:two\\n';\n",
            "      while [ ! -f .release-two ]; do sleep 0.02; done;\n",
            "      rmdir .console-running; touch two; printf 'DONE:two\\n'\n",
            "    console: true\n",
        ),
    )
    .context("write exclusive console manifest")?;

    let mut command = netsuke_build(&workspace, &["--jobs", "2", "build", "one", "two"])?;
    let child = command
        .spawn()
        .context("spawn Netsuke console exclusivity build")?;
    let mut cleanup = ChildCleanup::new(
        child,
        root.try_clone()?,
        vec![first_release.clone(), second_release.clone()],
    );
    let stdout = cleanup
        .child
        .stdout
        .take()
        .context("Netsuke child should expose stdout")?;
    let (output, reader) = spawn_line_reader(stdout);

    let first_start = wait_for_marker(&output, "START:")?;
    let first_output = parse_start_output(&first_start)?;
    release_edge(&root, &first_output)?;
    let second_start = wait_for_marker(&output, "START:")?;
    let second_output = parse_start_output(&second_start)?;
    ensure!(
        first_output != second_output,
        "the two console edges should each start once"
    );
    release_edge(&root, &second_output)?;
    finish_successfully(&mut cleanup)?;
    reader
        .join()
        .map_err(|_| anyhow::anyhow!("stdout forwarding thread panicked"))?;
    for target in ["one", "two"] {
        ensure!(
            root.exists(target),
            "console edge {target} should complete after release"
        );
    }
    ensure!(
        !root.exists(".console-running"),
        "the shared sentinel should be removed after both recipes finish"
    );
    Ok(())
}

/// Release one exclusivity-test recipe by its declared output name.
///
/// # Errors
///
/// Returns an error for unexpected output names or failed release-file writes.
fn release_edge(workspace: &Dir, output: &str) -> Result<()> {
    ensure!(
        matches!(output, "one" | "two"),
        "unexpected output marker {output}"
    );
    workspace
        .write(format!(".release-{output}"), b"release")
        .with_context(|| format!("release console edge {output}"))?;
    Ok(())
}

/// Extract and validate the target name printed by the shared recipe.
///
/// # Errors
///
/// Returns an error when the marker is absent or does not name a test output.
fn parse_start_output(line: &str) -> Result<String> {
    let output = line
        .split_once("START:")
        .map(|(_, output)| output.trim())
        .context("console recipe start marker should include its output")?;
    ensure!(
        matches!(output, "one" | "two"),
        "unexpected console output {output}"
    );
    Ok(output.to_owned())
}
