//! Fixtures and sandbox machinery for the `kani-check` boundary tests.
//!
//! The cases in the parent module all need the same three things: a temporary
//! directory written through a capability handle, a fake `cargo-kani`, and a
//! poisoned remote runner that fails loudly if the recipe ever tries to resolve
//! the shared CLI. The isolation case needs the extra machinery that applies
//! the read-only and no-network restrictions and reports whether each one took
//! effect.

use anyhow::{Context, Result, ensure};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, PermissionsExt};
#[cfg(target_os = "linux")]
use sandbox::SandboxPaths;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

// The sandbox is built from Linux namespaces and mount operations, so it is
// compiled only where the isolation case can run. Gating it here keeps a
// non-Linux Unix build free of dead-code warnings under `-D warnings`.
#[cfg(target_os = "linux")]
pub(super) mod sandbox;

/// The version pin every case compares against.
const PINNED_VERSION: &str = "0.67.0";

/// Quote a path so a POSIX shell reads it as one literal word.
fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

/// A temporary directory whose files are written through a capability handle.
pub(super) struct Fixture {
    temp: TempDir,
    directory: Dir,
}

impl Fixture {
    /// Create an empty fixture directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary directory cannot be created or opened.
    pub(super) fn new() -> Result<Self> {
        let temp = tempdir().context("create kani-check fixture")?;
        let directory = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .context("open kani-check fixture directory")?;
        Ok(Self { temp, directory })
    }

    /// Resolve a fixture-relative name to the path handed to `make`.
    pub(super) fn path(&self, name: &str) -> PathBuf {
        self.temp.path().join(name)
    }

    /// Return the fixture's temporary directory.
    ///
    /// Only the isolation case needs the directory itself rather than a named
    /// path inside it, and that case is Linux-only, so the accessor is too.
    #[cfg(target_os = "linux")]
    pub(super) fn root(&self) -> &Path {
        self.temp.path()
    }

    /// Create a directory inside the fixture.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    pub(super) fn create_dir_all(&self, name: &str) -> Result<()> {
        self.directory
            .create_dir_all(name)
            .with_context(|| format!("create fixture directory {name}"))
    }

    /// Write a shell script and mark it executable.
    ///
    /// # Errors
    ///
    /// Returns an error if the script cannot be written or marked executable.
    pub(super) fn write_executable(&self, name: &str, script: &str) -> Result<()> {
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
    pub(super) fn write_version_file(&self) -> Result<PathBuf> {
        self.write_version_file_pinning(PINNED_VERSION)
    }

    /// Write a version file pinning an explicit version.
    ///
    /// A case that pins something other than [`PINNED_VERSION`] proves the
    /// recipe reads the expected value from the file rather than carrying a
    /// copy of it.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub(super) fn write_version_file_pinning(&self, version: &str) -> Result<PathBuf> {
        self.directory
            .write("KANI_VERSION", version)
            .with_context(|| format!("write fake Kani version file pinning {version}"))?;
        Ok(self.path("KANI_VERSION"))
    }

    /// Read a fixture-relative file as UTF-8.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or is not valid UTF-8.
    pub(super) fn read_to_string(&self, name: &str) -> Result<String> {
        self.directory
            .read_to_string(name)
            .with_context(|| format!("read fixture file {name}"))
    }

    /// Create an executable fake verifier with the supplied version response.
    ///
    /// # Errors
    ///
    /// Returns an error if the fake binary cannot be created.
    pub(super) fn fake_kani(&self, script: &str) -> Result<PathBuf> {
        self.write_executable("cargo-kani", script)?;
        Ok(self.path("cargo-kani"))
    }

    /// Write the remote-runner poison and return it with its marker path.
    ///
    /// The poison both records that it ran and exits non-zero, so either a
    /// marker file or a failed query reveals an accidental invocation. It also
    /// names itself on standard error, which is the only signal available when
    /// the sandbox makes the marker file unwritable.
    ///
    /// # Errors
    ///
    /// Returns an error if the poison script cannot be created.
    fn poison_runner(&self) -> Result<(PathBuf, PathBuf)> {
        let marker = self.path("remote-runner-invoked");
        let poison = self.path("poison-runner");
        self.write_executable(
            "poison-runner",
            &format!(
                "#!/bin/sh\nprintf 'prover-tools runner invoked\\n' >&2\n\
                 printf invoked > '{}'\nexit 99\n",
                marker.display()
            ),
        )?;
        Ok((poison, marker))
    }

    /// Build a `make kani-check` command that poisons the remote runner.
    ///
    /// The returned path is the poison marker.
    ///
    /// # Errors
    ///
    /// Returns an error if the poison script cannot be created.
    pub(super) fn make_check_command(&self) -> Result<(Command, PathBuf)> {
        let (poison, poison_marker) = self.poison_runner()?;
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
    pub(super) fn ensure_remote_runner_unused(&self, marker: &Path) -> Result<()> {
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
    pub(super) fn run_check(&self, kani: &Path, version_file: &Path) -> Result<(Output, PathBuf)> {
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
    pub(super) fn run_fake_kani(&self, script: &str) -> Result<(Output, PathBuf)> {
        let kani = self.fake_kani(script)?;
        let version_file = self.write_version_file()?;
        self.run_check(&kani, &version_file)
    }

    /// Query the pinned version with `script` and return the failure diagnostic.
    ///
    /// # Errors
    ///
    /// Returns an error if the command cannot run or unexpectedly succeeds.
    pub(super) fn run_failing_check(&self, script: &str) -> Result<(String, PathBuf)> {
        let (output, marker) = self.run_fake_kani(script)?;
        ensure!(
            !output.status.success(),
            "the query should reject {script:?} against a 0.67.0 pin"
        );
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Ok((stderr, marker))
    }

    /// Query the pinned version inside a namespace with no network and no
    /// writable storage.
    ///
    /// The sandbox builds a chroot whose every directory is a read-only bind,
    /// so the inherited home directory, temporary paths, and any cache are
    /// equally unwritable rather than merely outside a narrow allowlist. The
    /// query runs in a fresh network namespace and process namespace. The
    /// script reports each restriction it applied, and reports one deliberately
    /// writable control path, so the caller can prove the sandbox was effective
    /// rather than assume it. The returned path is the poison marker.
    ///
    /// # Errors
    ///
    /// Returns an error if the fake verifier, version file, sandbox root, or
    /// sandbox script cannot be created, or if `unshare` cannot be spawned.
    #[cfg(target_os = "linux")]
    pub(super) fn run_sandboxed_check(&self, repo: &Path) -> Result<(Output, PathBuf)> {
        let (poison, marker) = self.poison_runner()?;
        let kani = self.fake_kani("#!/bin/sh\nprintf 'cargo-kani 0.67.0\\n'\n")?;
        let version_file = self.write_version_file()?;
        // The chroot root must outlive the child; keep its handle in scope.
        let chroot = tempdir().context("create kani-check sandbox root")?;
        let paths = SandboxPaths {
            repo,
            fixture: self.temp.path(),
            chroot: chroot.path(),
            kani: &kani,
            version_file: &version_file,
            poison: &poison,
        };
        let output = Command::new("unshare")
            .args([
                "--user",
                "--map-root-user",
                "--net",
                "--mount",
                "--pid",
                "--fork",
                "--",
                "sh",
                "-c",
            ])
            .arg(sandbox::script(&paths))
            .output()
            .context("run make kani-check inside an isolation namespace")?;
        Ok((output, marker))
    }
}
