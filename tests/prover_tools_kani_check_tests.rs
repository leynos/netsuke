//! Boundary tests for the local `kani-check` query.
//!
//! `make kani-check` must stay a local read-only query: it reads the pin and
//! asks the installed verifier for its version. These tests poison the remote
//! prover-tools runner so any attempt to resolve it fails loudly, proving the
//! query path cannot reach the network or write the uv cache.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

/// The version pin every case compares against.
const PINNED_VERSION: &str = "0.67.0";

/// A temporary directory whose files are written through a capability handle.
struct Fixture {
    temp: TempDir,
    directory: Dir,
}

impl Fixture {
    /// Create an empty fixture directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary directory cannot be created or opened.
    fn new() -> Result<Self> {
        let temp = tempdir().context("create kani-check fixture")?;
        let directory = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .context("open kani-check fixture directory")?;
        Ok(Self { temp, directory })
    }

    /// Resolve a fixture-relative name to the path handed to `make`.
    fn path(&self, name: &str) -> PathBuf {
        self.temp.path().join(name)
    }

    /// Write a shell script and mark it executable.
    ///
    /// # Errors
    ///
    /// Returns an error if the script cannot be written or marked executable.
    fn write_executable(&self, name: &str, script: &str) -> Result<()> {
        self.directory
            .write(name, script)
            .with_context(|| format!("write executable {name}"))?;
        let mut permissions = self
            .directory
            .metadata(name)
            .with_context(|| format!("stat executable {name}"))?
            .permissions();
        permissions.set_mode(0o755);
        self.directory
            .set_permissions(name, permissions)
            .with_context(|| format!("mark {name} executable"))
    }

    /// Write the pinned version file the query compares against.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    fn write_version_file(&self) -> Result<PathBuf> {
        self.directory
            .write("KANI_VERSION", PINNED_VERSION)
            .context("write fake Kani version file")?;
        Ok(self.path("KANI_VERSION"))
    }

    /// Create an executable fake verifier with the supplied version response.
    ///
    /// # Errors
    ///
    /// Returns an error if the fake binary cannot be created.
    fn fake_kani(&self, script: &str) -> Result<PathBuf> {
        self.write_executable("cargo-kani", script)?;
        Ok(self.path("cargo-kani"))
    }

    /// Build a `make kani-check` command that poisons the remote runner.
    ///
    /// The poison records that it ran and exits non-zero, so a successful query
    /// proves the recipe never invoked it. The returned path is the poison
    /// marker.
    ///
    /// # Errors
    ///
    /// Returns an error if the poison script cannot be created.
    fn make_check_command(&self) -> Result<(Command, PathBuf)> {
        let poison_marker = self.path("remote-runner-invoked");
        let poison = self.path("poison-runner");
        self.write_executable(
            "poison-runner",
            &format!(
                "#!/bin/sh\nprintf invoked > '{}'\nexit 99\n",
                poison_marker.display()
            ),
        )?;
        let mut command = Command::new("make");
        command
            .arg("--no-print-directory")
            .arg("-f")
            .arg("Makefile")
            .arg("kani-check")
            .arg(format!("PROVER_TOOLS={}", poison.display()))
            .arg(format!("UV={}", poison.display()));
        Ok((command, poison_marker))
    }

    /// Assert the query never reached the poisoned remote runner.
    ///
    /// # Errors
    ///
    /// Returns an error if the poison marker exists.
    fn ensure_remote_runner_unused(&self, marker: &Path) -> Result<()> {
        let name = marker
            .file_name()
            .and_then(|name| name.to_str())
            .context("poison marker should have a UTF-8 file name")?;
        ensure!(
            !self.directory.exists(name),
            "the query must not resolve the shared CLI through the remote runner"
        );
        Ok(())
    }

    /// Run `make kani-check` against the fake binary and version file.
    ///
    /// # Errors
    ///
    /// Returns an error if the command cannot be spawned.
    fn run_check(&self, kani: &Path, version_file: &Path) -> Result<(Output, PathBuf)> {
        let (mut command, marker) = self.make_check_command()?;
        let output = command
            .arg(format!("KANI={}", kani.display()))
            .arg(format!("KANI_VERSION_FILE={}", version_file.display()))
            .output()
            .context("run make kani-check")?;
        Ok((output, marker))
    }

    /// Install a fake verifier that prints `script` and query the pinned version.
    ///
    /// # Errors
    ///
    /// Returns an error if the fake binary, version file, or command cannot be
    /// created.
    fn run_fake_kani(&self, script: &str) -> Result<(Output, PathBuf)> {
        let kani = self.fake_kani(script)?;
        let version_file = self.write_version_file()?;
        self.run_check(&kani, &version_file)
    }

    /// Query the pinned version with `script` and return the failure diagnostic.
    ///
    /// # Errors
    ///
    /// Returns an error if the command cannot run or unexpectedly succeeds.
    fn run_failing_check(&self, script: &str) -> Result<(String, PathBuf)> {
        let (output, marker) = self.run_fake_kani(script)?;
        ensure!(
            !output.status.success(),
            "the query should reject {script:?} against a 0.67.0 pin"
        );
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Ok((stderr, marker))
    }
}

/// Check that a matching version succeeds and that the remote runner is never
/// consulted.
#[test]
fn kani_check_reads_the_installed_binary_without_remote_resolution() -> Result<()> {
    let fixture = Fixture::new()?;
    let (output, marker) = fixture.run_fake_kani("#!/bin/sh\nprintf 'cargo-kani 0.67.0\\n'\n")?;

    ensure!(
        output.status.success(),
        "make kani-check should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("Kani 0.67.0 matches"),
        "the query should report the matching version, got `{stdout}`"
    );
    fixture.ensure_remote_runner_unused(&marker)
}

/// Check that a version mismatch fails and names the expected and actual
/// versions.
#[test]
fn kani_check_reports_a_version_mismatch() -> Result<()> {
    let fixture = Fixture::new()?;
    let (stderr, marker) =
        fixture.run_failing_check("#!/bin/sh\nprintf 'cargo-kani 0.66.0\\n'\n")?;

    ensure!(
        stderr.contains("expected Kani 0.67.0") && stderr.contains("found 0.66.0"),
        "the mismatch diagnostic should name the expected and actual versions, got `{stderr}`"
    );
    fixture.ensure_remote_runner_unused(&marker)
}

/// Check that a failing verifier propagates its status through the recipe
/// diagnostic.
#[test]
fn kani_check_preserves_the_verifier_exit_status() -> Result<()> {
    let fixture = Fixture::new()?;
    let (stderr, marker) = fixture.run_failing_check("#!/bin/sh\nexit 42\n")?;

    ensure!(
        stderr.contains("prover-tools: target=kani-check failed exit=42"),
        "the diagnostic should preserve the verifier status, got `{stderr}`"
    );
    fixture.ensure_remote_runner_unused(&marker)
}

/// Check that version output without a semantic version is rejected rather than
/// compared loosely.
#[test]
fn kani_check_rejects_unparseable_version_output() -> Result<()> {
    let fixture = Fixture::new()?;
    let (stderr, marker) = fixture.run_failing_check("#!/bin/sh\nprintf 'unknown build\\n'\n")?;

    ensure!(
        stderr.contains("could not parse Kani version"),
        "the diagnostic should report the unparseable output, got `{stderr}`"
    );
    fixture.ensure_remote_runner_unused(&marker)
}

/// Check that the default active-home command survives an apostrophe in
/// `CARGO_HOME`.
#[test]
fn kani_check_handles_apostrophes_in_the_active_cargo_home() -> Result<()> {
    let fixture = Fixture::new()?;
    let cargo_home = fixture.path("cargo's home with spaces");
    fixture
        .directory
        .create_dir_all("cargo's home with spaces/bin")
        .context("create active Cargo bin directory")?;
    fixture.write_executable(
        "cargo's home with spaces/bin/cargo-kani",
        "#!/bin/sh\nprintf 'cargo-kani 0.67.0\\n'\n",
    )?;
    let version_file = fixture.write_version_file()?;
    let (mut command, marker) = fixture.make_check_command()?;
    let output = command
        .arg(format!("KANI_VERSION_FILE={}", version_file.display()))
        .arg(format!("CARGO_HOME={}", cargo_home.display()))
        .output()
        .context("run make kani-check with an apostrophe in CARGO_HOME")?;

    ensure!(
        output.status.success(),
        "make kani-check should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fixture.ensure_remote_runner_unused(&marker)
}
