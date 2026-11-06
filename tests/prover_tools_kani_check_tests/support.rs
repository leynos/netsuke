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
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

/// The version pin every case compares against.
const PINNED_VERSION: &str = "0.67.0";

/// Environment variable that turns an unavailable isolation sandbox into a
/// failure instead of a reported skip.
const SANDBOX_STRICT_ENV: &str = "NETSUKE_KANI_CHECK_SANDBOX_STRICT";

/// Paths the isolation sandbox script interpolates.
///
/// Grouping them keeps the script builder's signature small, and makes the
/// provenance of each interpolation explicit at the call site.
struct SandboxPaths<'a> {
    repo: &'a Path,
    root: &'a Path,
    kani: &'a Path,
    version_file: &'a Path,
    poison: &'a Path,
}

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
    /// The repository root and the fixture directory are both rebound
    /// read-only, and the query runs in a fresh network namespace. The script
    /// reports each restriction it applied, so the caller can prove the
    /// sandbox was effective rather than assume it. The returned path is the
    /// poison marker.
    ///
    /// # Errors
    ///
    /// Returns an error if the fake verifier, version file, or sandbox script
    /// cannot be created, or if `unshare` cannot be spawned.
    pub(super) fn run_sandboxed_check(&self, repo: &Path) -> Result<(Output, PathBuf)> {
        let (poison, marker) = self.poison_runner()?;
        let kani = self.fake_kani("#!/bin/sh\nprintf 'cargo-kani 0.67.0\\n'\n")?;
        let version_file = self.write_version_file()?;
        let paths = SandboxPaths {
            repo,
            root: self.temp.path(),
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
                "--",
                "sh",
                "-c",
            ])
            .arg(sandbox_script(&paths))
            .output()
            .context("run make kani-check inside an isolation namespace")?;
        Ok((output, marker))
    }
}

/// Build the shell script that applies the restrictions and runs the query.
///
/// Each restriction is asserted, not merely attempted: a sandbox that fails to
/// take effect would otherwise make the boundary test pass vacuously. `mount`
/// needs a self-bind first because `/` itself cannot be remounted read-only
/// from inside a user namespace.
fn sandbox_script(paths: &SandboxPaths<'_>) -> String {
    let repo = shell_quote(paths.repo);
    let root = shell_quote(paths.root);
    let kani = shell_quote(paths.kani);
    let version_file = shell_quote(paths.version_file);
    let poison = shell_quote(paths.poison);
    format!(
        concat!(
            "set -u\n",
            "mount --bind {repo} {repo} && mount -o remount,ro,bind {repo} || exit 90\n",
            "mount --bind {root} {root} && mount -o remount,ro,bind {root} || exit 91\n",
            "cd {repo} || exit 92\n",
            "if touch .kani-check-write-probe 2>/dev/null; then echo 'repo=writable'; exit 93; fi\n",
            "echo 'repo=read-only'\n",
            "if touch write-probe 2>/dev/null; then echo 'temp=writable'; exit 94; fi\n",
            "echo 'temp=read-only'\n",
            "echo \"ifaces=$(cut -d: -f1 /proc/net/dev | tail -n +3 | tr -d ' ' | tr '\\n' ',')\"\n",
            "if timeout 5 getent hosts example.com >/dev/null 2>&1; ",
            "then echo 'dns=resolved'; exit 95; fi\n",
            "echo 'dns=blocked'\n",
            "exec make --no-print-directory kani-check KANI={kani} ",
            "KANI_VERSION_FILE={version_file} PROVER_TOOLS={poison} UV={poison}\n",
        ),
        repo = repo,
        root = root,
        kani = kani,
        version_file = version_file,
        poison = poison,
    )
}

/// Report why the isolation sandbox cannot run, if it cannot.
///
/// A host without user namespaces cannot demonstrate the blocked state. The
/// case reports that as a skip, which strict mode turns back into a failure so
/// a lane that requires the boundary test cannot pass without exercising it.
pub(super) fn sandbox_unavailable() -> Option<String> {
    let probe = Command::new("unshare")
        .args([
            "--user",
            "--map-root-user",
            "--net",
            "--mount",
            "--",
            "true",
        ])
        .output();
    match probe {
        Ok(output) if output.status.success() => None,
        Ok(output) => Some(format!(
            "unshare could not create an isolation namespace: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(error) => Some(format!("unshare is not runnable: {error}")),
    }
}

/// Report a skipped isolation case, unless strict mode forbids skipping.
///
/// # Errors
///
/// Returns an error when [`SANDBOX_STRICT_ENV`] is `1`, so a lane that requires
/// the boundary test fails rather than silently passing on a host that could
/// not run it.
#[expect(
    clippy::disallowed_methods,
    reason = "this is the single site that reads the strict-mode switch; the \
              test binary has no injected environment to consult"
)]
#[expect(
    clippy::print_stderr,
    reason = "a skipped prerequisite must be visible in the captured test output"
)]
pub(super) fn skip_sandbox(reason: &str) -> Result<()> {
    let strict = std::env::var_os(SANDBOX_STRICT_ENV).is_some_and(|value| value == "1");
    ensure!(
        !strict,
        "the kani-check isolation sandbox could not be exercised: {reason} \
         (strict mode requires the boundary test to run for real)"
    );
    eprintln!("skipped: kani-check isolation — {reason}");
    Ok(())
}
