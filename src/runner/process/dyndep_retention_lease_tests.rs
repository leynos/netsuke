//! Cross-process tests for the dyndep publication lease.

use super::*;
use fs4::FileExt;
use mockable::{DefaultEnv, Env};
use rstest::rstest;
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

const LEASE_WORKER_DIRECTORY: &str = "NETSUKE_TEST_DYNDEP_LEASE_DIRECTORY";
const LEASE_WORKER_NAME: &str = "runner::process::dyndep_retention::retention_tests::lease_tests::dyndep_publication_lease_worker";
const WORKER_MARKER_PREFIX: &str = "netsuke-dyndep-lease:";
const ACTIVE_SIDECAR_PATH: &str = ".netsuke/dyndep/active.dd";
const ACTIVE_SIDECAR_CONTENT: &str = "active";

fn write_worker_marker(mut writer: impl Write, marker: &str) -> Result<()> {
    writeln!(writer, "{WORKER_MARKER_PREFIX}{marker}")?;
    writer.flush()?;
    Ok(())
}

fn wait_for_worker_marker(reader: &mut impl BufRead, marker: &str) -> Result<()> {
    let expected = format!("{WORKER_MARKER_PREFIX}{marker}");
    loop {
        let mut line = String::new();
        ensure!(
            reader.read_line(&mut line)? != 0,
            "lease worker closed stdout before emitting {expected}"
        );
        if line.trim_end().ends_with(&expected) {
            return Ok(());
        }
    }
}

fn worker_directory() -> Result<Dir> {
    let process_env = DefaultEnv;
    let root = PathBuf::from(
        process_env
            .os_string(LEASE_WORKER_DIRECTORY)
            .context("read lease-worker directory")?,
    );
    let path = Utf8PathBuf::from_path_buf(root).map_err(|path| {
        anyhow::anyhow!("lease-worker directory is not UTF-8: {}", path.display())
    })?;
    Dir::open_ambient_dir(path, cap_std::ambient_authority()).map_err(Into::into)
}

fn assert_lease_is_unavailable(dir: &Dir) -> Result<()> {
    let mut options = cap_std::fs_utf8::OpenOptions::new();
    options.read(true).write(true);
    let file = dir.open_with(DYNDEP_LOCK, &options)?.into_std();
    let error =
        FileExt::try_lock(&file).expect_err("the parent lease must block the child lock attempt");
    ensure!(
        matches!(error, fs4::TryLockError::WouldBlock),
        "the parent lease must make the child lock attempt nonblocking: {error}"
    );
    Ok(())
}

#[rstest]
fn lease_blocks_other_processes_until_the_active_sidecar_is_released(
    dyndep_workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (temp, dir) = dyndep_workspace?;
    let active = sidecar(ACTIVE_SIDECAR_PATH, ACTIVE_SIDECAR_CONTENT);
    let publication_lease = materialize_dyndep_files(&dir, std::slice::from_ref(&active))?;
    drop(publication_lease);
    let lease = DyndepPublicationLease::acquire(&dir)?;
    let mut child = Command::new(std::env::current_exe()?)
        .args(["--ignored", "--exact", LEASE_WORKER_NAME, "--nocapture"])
        .env(LEASE_WORKER_DIRECTORY, temp.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    {
        let child_stdout = child
            .stdout
            .as_mut()
            .context("capture lease-worker stdout")?;
        let mut stdout = BufReader::new(child_stdout);
        wait_for_worker_marker(&mut stdout, "blocked")?;
        drop(lease);
        wait_for_worker_marker(&mut stdout, "completed")?;
    }
    let output = child.wait_with_output()?;
    ensure!(
        output.status.success(),
        "lease worker failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        dir.open(active.relative_path()).is_ok(),
        "the original active sidecar must remain after the child retention pass"
    );
    Ok(())
}

#[test]
#[ignore = "invoked by the cross-process dyndep lease test"]
fn dyndep_publication_lease_worker() -> Result<()> {
    let dir = worker_directory()?;
    assert_lease_is_unavailable(&dir)?;
    write_worker_marker(std::io::stdout().lock(), "blocked")?;

    let active = sidecar(ACTIVE_SIDECAR_PATH, ACTIVE_SIDECAR_CONTENT);
    let lease = materialize_dyndep_files(&dir, std::slice::from_ref(&active))?;
    lease.prune(&dir, std::slice::from_ref(&active))?;
    write_worker_marker(std::io::stdout().lock(), "completed")
}
