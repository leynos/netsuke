#![cfg(unix)]
//! Process-boundary RSS coverage for bounded Ninja status parsing.
//!
//! The test redirects the real CLI's large child-output stream to disk, then
//! runs a fresh worker process for each measurement. Each worker has exactly
//! one `netsuke` child and records `RUSAGE_CHILDREN`, keeping the test harness
//! from retaining the payload it is designed to bound.

use anyhow::{Context, Result, ensure};
use assert_cmd::cargo::cargo_bin;
use cap_std::{ambient_authority, fs::Dir};
use mockable::{DefaultEnv, Env};
use nix::sys::resource::{UsageWho, getrusage};
use rstest::{fixture, rstest};
use std::{
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use tempfile::{TempDir, tempdir};
use test_support::{fluent::normalize_fluent_isolates, fs as test_fs, write_exec_with_content};

/// Bytes written before the fake Ninja's terminating newline.
const OVERSIZED_LINE_BYTES: usize = 256 * 1024 * 1024;
/// Bytes accepted beyond plain forwarding for allocator and loader variation.
const RSS_OVERHEAD_ALLOWANCE_BYTES: u64 = 16 * 1024 * 1024;
/// Status output emitted immediately after the oversized fake-Ninja line.
const RESUMED_STATUS: &[u8] = b"\n[1/2] cc -c resumed.c\n";
/// Initial output bytes that identify the large fake-Ninja stream.
const OUTPUT_PREFIX: &[u8] = b"[xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
/// Worker environment key carrying the temporary Netsuke workspace.
const WORKER_WORKSPACE: &str = "NETSUKE_RSS_WORKSPACE";
/// Worker environment key carrying the fake Ninja executable path.
const WORKER_NINJA: &str = "NETSUKE_RSS_NINJA";
/// Worker environment key selecting the CLI progress policy.
const WORKER_PROGRESS: &str = "NETSUKE_RSS_PROGRESS";
/// Worker environment key carrying the redirected child stdout path.
const WORKER_STDOUT: &str = "NETSUKE_RSS_STDOUT";
/// Worker environment key carrying the captured child stderr path.
const WORKER_STDERR: &str = "NETSUKE_RSS_STDERR";
/// Worker environment key carrying the direct-child RSS report path.
const WORKER_RSS: &str = "NETSUKE_RSS_VALUE";
/// Name of the ignored test used as the isolated resource-measurement worker.
const WORKER_TEST_NAME: &str = "resource_observation_worker";

/// Own the workspace and injected Ninja program for one process-boundary run.
struct LargeOutputFixture {
    /// Temporary workspace retained until the test has inspected its files.
    workspace: TempDir,
    /// Fake Ninja program that produces the large unterminated line.
    ninja_program: PathBuf,
}

/// Hold the bounded observations collected from one isolated worker process.
struct NetsukeProcessObservation {
    /// File receiving the CLI's forwarded standard output.
    stdout_path: PathBuf,
    /// Captured CLI standard error, which contains the progress update.
    stderr: String,
    /// Peak resident-set size reported for the worker's sole Netsuke child.
    peak_rss_bytes: u64,
}

/// Create a workspace and fake Ninja that emits one 256 MiB status-like line.
#[fixture]
fn large_output_fixture() -> Result<LargeOutputFixture> {
    let workspace = tempdir().context("create large-output workspace")?;
    test_fs::copy(
        "tests/data/minimal.yml",
        workspace.path().join("Netsukefile"),
    )
    .context("copy minimal manifest into large-output workspace")?;
    let script = concat!(
        "#!/bin/sh\n",
        "printf '['\n",
        "head -c 268435455 /dev/zero | tr '\\000' x\n",
        "printf '\\n[1/2] cc -c resumed.c\\n'\n",
    );
    let ninja_program = write_exec_with_content(workspace.path(), "large-output-ninja", script)
        .context("write large-output fake Ninja executable")?;

    Ok(LargeOutputFixture {
        workspace,
        ninja_program,
    })
}

/// Run a resource worker that sends its Netsuke child's stdout to `stdout_name`.
fn run_netsuke_with_progress(
    fixture: &LargeOutputFixture,
    progress: &str,
    stdout_name: &str,
) -> Result<NetsukeProcessObservation> {
    let stdout_path = fixture.workspace.path().join(stdout_name);
    let stderr_path = fixture
        .workspace
        .path()
        .join(format!("{stdout_name}.stderr"));
    let rss_path = fixture.workspace.path().join(format!("{stdout_name}.rss"));
    let worker = std::env::current_exe().context("locate RSS test worker executable")?;
    let status = Command::new(worker)
        .args(["--ignored", "--exact", WORKER_TEST_NAME, "--nocapture"])
        .env(WORKER_WORKSPACE, fixture.workspace.path())
        .env(WORKER_NINJA, &fixture.ninja_program)
        .env(WORKER_PROGRESS, progress)
        .env(WORKER_STDOUT, &stdout_path)
        .env(WORKER_STDERR, &stderr_path)
        .env(WORKER_RSS, &rss_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("run isolated Netsuke RSS worker")?;
    ensure!(
        status.success(),
        "isolated Netsuke RSS worker should succeed: {status}"
    );
    let stderr = test_fs::read_to_string(&stderr_path)
        .with_context(|| format!("read captured Netsuke stderr {}", stderr_path.display()))?;
    let peak_rss_bytes = test_fs::read_to_string(&rss_path)
        .with_context(|| format!("read Netsuke RSS report {}", rss_path.display()))?
        .trim()
        .parse()
        .context("parse Netsuke RSS report")?;

    Ok(NetsukeProcessObservation {
        stdout_path,
        stderr,
        peak_rss_bytes,
    })
}

/// Return the isolated worker's child-process peak RSS in bytes.
fn child_peak_rss_bytes() -> Result<u64> {
    let usage = getrusage(UsageWho::RUSAGE_CHILDREN).map_err(io::Error::other)?;
    let resident_set =
        u64::try_from(usage.max_rss()).context("getrusage returned a negative RSS")?;
    #[cfg(target_os = "macos")]
    let bytes = resident_set;
    #[cfg(not(target_os = "macos"))]
    let bytes = resident_set.saturating_mul(1024);
    Ok(bytes)
}

/// Read one required path from the isolated worker's supplied environment.
fn worker_path(name: &str) -> Result<PathBuf> {
    DefaultEnv
        .path_buf(name)
        .with_context(|| format!("resource worker requires {name}"))
}

/// Read the isolated worker's explicit progress policy.
fn worker_progress() -> Result<String> {
    DefaultEnv
        .string(WORKER_PROGRESS)
        .context("resource worker requires progress policy")
}

/// Open the capability directory containing a worker output path.
fn output_directory(path: &Path) -> Result<Dir> {
    let parent = path
        .parent()
        .context("worker output path should have a parent directory")?;
    Dir::open_ambient_dir(parent, ambient_authority())
        .with_context(|| format!("open worker output directory {}", parent.display()))
}

/// Extract the filename component from a worker output path.
fn output_name(path: &Path) -> Result<&Path> {
    path.file_name()
        .map(Path::new)
        .with_context(|| format!("worker output path should name a file: {}", path.display()))
}

/// Create a worker output file through the capability filesystem boundary.
fn create_output_file(path: &Path) -> Result<cap_std::fs::File> {
    output_directory(path)?
        .create(output_name(path)?)
        .with_context(|| format!("create worker output file {}", path.display()))
}

/// Open a worker output file through the capability filesystem boundary.
fn open_output_file(path: &Path) -> Result<cap_std::fs::File> {
    output_directory(path)?
        .open(output_name(path)?)
        .with_context(|| format!("open worker output file {}", path.display()))
}

/// Run the sole Netsuke child for the resource-measurement worker.
#[test]
#[ignore = "invoked by the process-boundary RSS test"]
fn resource_observation_worker() -> Result<()> {
    let workspace = worker_path(WORKER_WORKSPACE)?;
    let ninja_program = worker_path(WORKER_NINJA)?;
    let stdout_path = worker_path(WORKER_STDOUT)?;
    let stderr_path = worker_path(WORKER_STDERR)?;
    let rss_path = worker_path(WORKER_RSS)?;
    let progress = worker_progress()?;
    let stdout = create_output_file(&stdout_path)?;
    let mut command = Command::new(cargo_bin!("netsuke"));
    command
        .current_dir(&workspace)
        .env("NETSUKE_NINJA", ninja_program)
        .env("HOME", &workspace)
        .env("XDG_CONFIG_HOME", workspace.join(".config"))
        .env_remove("NETSUKE_CONFIG")
        .env_remove("NETSUKE_CONFIG_PATH")
        .env_remove("NETSUKE_OUTPUT_FORMAT")
        .args(["--accessibility", "on", "--progress", &progress, "build"])
        .stdout(Stdio::from(stdout.into_std()))
        .stderr(Stdio::piped());
    let mut child = command.spawn().context("spawn Netsuke build process")?;
    let mut stderr = child
        .stderr
        .take()
        .context("Netsuke build process should expose stderr")?;
    let exit_status = child.wait().context("wait for Netsuke build process")?;
    let mut stderr_text = String::new();
    stderr
        .read_to_string(&mut stderr_text)
        .context("read bounded Netsuke stderr")?;
    test_fs::write(&stderr_path, &stderr_text)
        .with_context(|| format!("write captured Netsuke stderr {}", stderr_path.display()))?;
    ensure!(
        exit_status.success(),
        "Netsuke build should succeed: {exit_status}; stderr: {stderr_text}"
    );
    test_fs::write(&rss_path, child_peak_rss_bytes()?.to_string())
        .with_context(|| format!("write Netsuke RSS report {}", rss_path.display()))?;
    Ok(())
}

/// Verify the disk-streamed output without constructing the large payload.
fn assert_large_output(path: &Path) -> Result<()> {
    let mut output = open_output_file(path)?;
    let payload_bytes =
        u64::try_from(OVERSIZED_LINE_BYTES).context("payload length should fit u64")?;
    let suffix_bytes =
        u64::try_from(RESUMED_STATUS.len()).context("status length should fit u64")?;
    let expected_length = payload_bytes.saturating_add(suffix_bytes);
    ensure!(
        output.metadata()?.len() == expected_length,
        "forwarded output length should be {expected_length} bytes"
    );

    let mut prefix = [0_u8; OUTPUT_PREFIX.len()];
    output
        .read_exact(&mut prefix)
        .context("read first output bytes")?;
    ensure!(prefix == OUTPUT_PREFIX, "forwarded output prefix changed");

    output
        .seek(SeekFrom::Start(payload_bytes.saturating_sub(1)))
        .context("seek to output newline boundary")?;
    let mut boundary = vec![0_u8; RESUMED_STATUS.len().saturating_add(1)];
    output
        .read_exact(&mut boundary)
        .context("read output newline and resumed status")?;
    let mut expected_boundary = vec![b'x'];
    expected_boundary.extend_from_slice(RESUMED_STATUS);
    ensure!(
        boundary == expected_boundary,
        "forwarded output should retain the final payload byte, newline, and resumed status"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn status_parsing_rss_stays_bounded_at_the_process_boundary(
    #[from(large_output_fixture)] fixture_result: Result<LargeOutputFixture>,
) -> Result<()> {
    let fixture = fixture_result?;
    let plain = run_netsuke_with_progress(&fixture, "never", "plain.stdout")?;
    let status = run_netsuke_with_progress(&fixture, "always", "status.stdout")?;

    assert_large_output(&plain.stdout_path)?;
    assert_large_output(&status.stdout_path)?;
    ensure!(
        normalize_fluent_isolates(&status.stderr).contains("Task 1/2"),
        "progress parser should resume after the oversized line: {}",
        status.stderr
    );
    ensure!(
        status.peak_rss_bytes
            <= plain
                .peak_rss_bytes
                .saturating_add(RSS_OVERHEAD_ALLOWANCE_BYTES),
        "status parsing RSS {} exceeded plain forwarding RSS {} by more than the {} byte allowance",
        status.peak_rss_bytes,
        plain.peak_rss_bytes,
        RSS_OVERHEAD_ALLOWANCE_BYTES,
    );
    Ok(())
}
