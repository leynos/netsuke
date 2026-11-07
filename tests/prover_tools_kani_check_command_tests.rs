//! Behavioural tests for the `kani-check` version-drift guard.
//!
//! `make kani-check` reads the pinned version from `KANI_VERSION_FILE`, asks
//! the installed verifier for `--version`, and compares the two. These tests
//! drive the recipe with a fake verifier so every branch is exercised without
//! installing Kani: a match, a mismatch, a missing pin, and a pin that cannot
//! be read at all.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

/// A fake verifier plus the files the recipe reads.
struct FakeKani {
    _temp: TempDir,
    executable: PathBuf,
    invocation_log: PathBuf,
    version_file: PathBuf,
}

impl FakeKani {
    /// Create a fake verifier that reports `reported_version`.
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary directory, the fake executable, or the
    /// version file cannot be created.
    fn new(pinned_version: &str, reported_version: &str) -> Result<Self> {
        let temp = tempdir().context("create fake Kani directory")?;
        let executable = temp.path().join("fake-kani");
        let invocation_log = temp.path().join("invocations.log");
        let version_file = temp.path().join("KANI_VERSION");
        write_fake_kani(&executable, &invocation_log, reported_version)?;
        fs::write(&version_file, pinned_version).context("write fake Kani version file")?;

        Ok(Self {
            _temp: temp,
            executable,
            invocation_log,
            version_file,
        })
    }

    /// Create a fake verifier that fails with `status` after printing `noise`.
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary directory, the fake executable, or the
    /// version file cannot be created.
    fn failing(pinned_version: &str, status: i32, noise: &str) -> Result<Self> {
        let temp = tempdir().context("create fake Kani directory")?;
        let executable = temp.path().join("fake-kani");
        let invocation_log = temp.path().join("invocations.log");
        let version_file = temp.path().join("KANI_VERSION");
        write_failing_kani(&executable, &invocation_log, status, noise)?;
        fs::write(&version_file, pinned_version).context("write fake Kani version file")?;

        Ok(Self {
            _temp: temp,
            executable,
            invocation_log,
            version_file,
        })
    }

    /// Read the arguments the fake verifier received.
    ///
    /// An absent log means the verifier was never invoked.
    ///
    /// # Errors
    ///
    /// Returns an error if the invocation log exists but cannot be read.
    fn invocation_args(&self) -> Result<Vec<String>> {
        match fs::read_to_string(&self.invocation_log) {
            Ok(contents) => Ok(contents.lines().map(str::to_owned).collect()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => {
                Err(error).with_context(|| format!("read {}", self.invocation_log.display()))
            }
        }
    }
}

/// Write an executable that records its arguments and prints a version.
///
/// # Errors
///
/// Returns an error if the script cannot be written or marked executable.
fn write_fake_kani(executable: &Path, invocation_log: &Path, reported_version: &str) -> Result<()> {
    let script = format!(
        r#"#!/bin/sh
for argument do
    printf '%s\n' "$argument" >> '{}'
done
printf 'kani {}\n'
"#,
        invocation_log.display(),
        reported_version
    );
    write_executable(executable, &script)
}

/// Write an executable that records its arguments, complains, and fails.
///
/// # Errors
///
/// Returns an error if the script cannot be written or marked executable.
fn write_failing_kani(
    executable: &Path,
    invocation_log: &Path,
    status: i32,
    noise: &str,
) -> Result<()> {
    let script = format!(
        r#"#!/bin/sh
for argument do
    printf '%s\n' "$argument" >> '{}'
done
printf '%s\n' '{}' >&2
exit {}
"#,
        invocation_log.display(),
        noise,
        status
    );
    write_executable(executable, &script)
}

/// Write `script` to `executable` and mark it executable.
///
/// # Errors
///
/// Returns an error if the script cannot be written or its mode cannot be set.
fn write_executable(executable: &Path, script: &str) -> Result<()> {
    fs::write(executable, script)
        .with_context(|| format!("write fake Kani executable to {}", executable.display()))?;
    let mut permissions = fs::metadata(executable)
        .with_context(|| format!("stat fake Kani executable {}", executable.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(executable, permissions)
        .with_context(|| format!("mark {} executable", executable.display()))
}

/// Run `make kani-check` against the fake verifier and version file.
///
/// # Errors
///
/// Returns an error if the Make invocation cannot be spawned.
fn run_kani_check(fake: &FakeKani) -> Result<Output> {
    Command::new("make")
        .arg("--no-print-directory")
        .arg("-f")
        .arg("Makefile")
        .arg(format!("KANI={}", fake.executable.display()))
        .arg(format!("KANI_VERSION_FILE={}", fake.version_file.display()))
        .arg("kani-check")
        .output()
        .context("run make kani-check")
}

/// Check that a matching pin succeeds and that the verifier is asked for its
/// version rather than being run as a proof command.
#[test]
fn kani_check_accepts_a_matching_version_and_queries_the_verifier() -> Result<()> {
    let fake = FakeKani::new("0.67.0", "0.67.0")?;

    let output = run_kani_check(&fake)?;

    ensure!(
        output.status.success(),
        "make kani-check should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        fake.invocation_args()? == ["--version"],
        "the verifier should receive exactly `--version`, got `{:?}`",
        fake.invocation_args()?
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("Kani 0.67.0 matches"),
        "the recipe should report the match, got `{stdout}`"
    );
    Ok(())
}

/// Check that a pin naming a different release fails and names both versions.
#[test]
fn kani_check_rejects_a_different_pinned_version() -> Result<()> {
    let fake = FakeKani::new("0.66.0", "0.67.0")?;

    let output = run_kani_check(&fake)?;

    ensure!(!output.status.success(), "make kani-check should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("category=version-mismatch"),
        "the diagnostic should carry a stable category, got `{stderr}`"
    );
    ensure!(
        stderr.contains("expected Kani 0.66.0"),
        "the diagnostic should name the pinned version, got `{stderr}`"
    );
    ensure!(
        stderr.contains("found 0.67.0"),
        "the diagnostic should name the installed version, got `{stderr}`"
    );
    Ok(())
}

/// Check that a missing pin fails before the verifier is consulted.
#[test]
fn kani_check_reports_a_missing_version_file() -> Result<()> {
    let fake = FakeKani::new("0.67.0", "0.67.0")?;
    fs::remove_file(&fake.version_file).context("remove fake Kani version file")?;

    let output = run_kani_check(&fake)?;

    ensure!(!output.status.success(), "make kani-check should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("category=pin-unreadable"),
        "the diagnostic should carry a stable category, got `{stderr}`"
    );
    ensure!(
        stderr.contains("could not read"),
        "the diagnostic should report the unreadable pin, got `{stderr}`"
    );
    ensure!(
        fake.invocation_args()?.is_empty(),
        "the verifier must not run when the pin cannot be read"
    );
    Ok(())
}

/// Check that a pin path that exists but cannot be read as a file fails.
///
/// A directory is used rather than a permission-denied file so the case holds
/// even when the suite runs as a user that can read anything.
#[test]
fn kani_check_reports_a_version_path_that_is_not_a_file() -> Result<()> {
    let fake = FakeKani::new("0.67.0", "0.67.0")?;
    fs::remove_file(&fake.version_file).context("remove fake Kani version file")?;
    fs::create_dir(&fake.version_file).context("replace the pin with a directory")?;

    let output = run_kani_check(&fake)?;

    ensure!(!output.status.success(), "make kani-check should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("category=pin-unreadable"),
        "the diagnostic should carry a stable category, got `{stderr}`"
    );
    ensure!(
        stderr.contains("could not read"),
        "the diagnostic should report the unreadable pin, got `{stderr}`"
    );
    Ok(())
}

/// Check that a failing verifier yields a category, its status, and a bounded
/// diagnostic with control characters removed.
///
/// The verifier is an external binary, so its output is untrusted: a stray
/// escape sequence could recolour or forge the surrounding log, and an
/// unbounded message could flood it. The recipe must therefore strip control
/// characters and cap the quote before printing it.
#[test]
fn kani_check_reports_a_sanitized_diagnostic_when_the_verifier_fails() -> Result<()> {
    let noise = format!("kani 9.9.9 \u{1b}[31m{}\ttail", "x".repeat(300));
    let fake = FakeKani::failing("0.67.0", 9, &noise)?;

    let output = run_kani_check(&fake)?;

    ensure!(!output.status.success(), "make kani-check should fail");
    ensure!(
        fake.invocation_args()? == ["--version"],
        "a failing verifier should still have been asked for its version"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("category=verifier-failed"),
        "the diagnostic should carry a stable category, got `{stderr}`"
    );
    ensure!(
        stderr.contains("failed exit=9"),
        "the diagnostic should report the verifier's status, got `{stderr}`"
    );
    let diagnostic = stderr
        .split("diagnostic=")
        .nth(1)
        .context("the failing verifier's diagnostic should be reported")?
        .lines()
        .next()
        .context("the diagnostic should occupy its own line")?;
    ensure!(
        diagnostic.chars().count() <= 200,
        "the diagnostic should be capped at 200 characters, got {}",
        diagnostic.chars().count()
    );
    ensure!(
        !diagnostic.chars().any(char::is_control),
        "the diagnostic should contain no control characters, got `{diagnostic}`"
    );
    ensure!(
        diagnostic.contains("kani 9.9.9"),
        "the diagnostic should keep the useful prefix, got `{diagnostic}`"
    );
    Ok(())
}
