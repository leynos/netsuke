//! The fake verifier and file layout the `kani-check` cases drive.
//!
//! Every case in the parent module needs the same three things: a `make
//! kani-check` invocation that names the fixture's own pin, home and executable;
//! a verifier that answers `--version` in a chosen way while recording that it
//! ran; and a way to read those recordings back. Keeping them here leaves the
//! parent module to state the cases, which is what keeps either file inside the
//! size cap.

use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

/// How the fake verifier answers a `--version` request.
#[derive(Clone, Copy)]
pub(crate) enum Behaviour<'a> {
    /// Print `version` and succeed.
    Reports(&'a str),
    /// Print `noise` to standard error and exit with `status`.
    Fails { status: i32, noise: &'a str },
}

/// A fake verifier plus the files the recipe reads.
pub(crate) struct FakeKani {
    temp: TempDir,
    executable: PathBuf,
    invocation_log: PathBuf,
    pub(crate) version_file: PathBuf,
    kani_home: PathBuf,
}

impl FakeKani {
    /// Create a fake verifier that answers `--version` as `behaviour` describes.
    ///
    /// The fixture also supplies the Kani home the recipe inspects before it
    /// runs anything: the home carries an installed payload for the pinned
    /// version, because without one `cargo-kani` would treat the installation
    /// as incomplete and start a download. A home without that payload is the
    /// separate refusal case in `prover_tools_kani_check_tests`.
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary directory, the fake executable, the
    /// version file, or the Kani payload directory cannot be created.
    pub(crate) fn new(pinned_version: &str, behaviour: Behaviour<'_>) -> Result<Self> {
        let temp = tempdir().context("create fake Kani directory")?;
        let executable = temp.path().join("fake-kani");
        let invocation_log = temp.path().join("invocations.log");
        let version_file = temp.path().join("KANI_VERSION");
        let kani_home = temp.path().join("kani-home");
        write_fake_kani(&executable, &invocation_log, behaviour)?;
        fs::write(&version_file, pinned_version).context("write fake Kani version file")?;
        fs::create_dir_all(kani_home.join(format!("kani-{pinned_version}")))
            .context("create the fake Kani payload directory")?;

        Ok(Self {
            temp,
            executable,
            invocation_log,
            version_file,
            kani_home,
        })
    }

    /// Resolve a name inside the fixture directory.
    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.temp.path().join(name)
    }

    /// Read the arguments the fake verifier received.
    ///
    /// An absent log means the verifier was never invoked.
    ///
    /// # Errors
    ///
    /// Returns an error if the invocation log exists but cannot be read.
    pub(crate) fn invocation_args(&self) -> Result<Vec<String>> {
        match fs::read_to_string(&self.invocation_log) {
            Ok(contents) => Ok(contents.lines().map(str::to_owned).collect()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => {
                Err(error).with_context(|| format!("read {}", self.invocation_log.display()))
            }
        }
    }

    /// Write a recorder that marks its invocation before failing.
    ///
    /// The recorder stands in for a network-capable tool. It appends to a
    /// marker file *before* exiting non-zero, so an attempt is visible even
    /// when the caller ignores the failure.
    ///
    /// # Errors
    ///
    /// Returns an error if the recorder cannot be written or marked executable.
    pub(crate) fn write_recorder(&self) -> Result<(PathBuf, PathBuf)> {
        let marker = self.path("network-attempt.log");
        let recorder = self.path("network-recorder");
        write_executable(
            &recorder,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$0\" >> '{}'\nexit 99\n",
                marker.display()
            ),
        )?;
        Ok((recorder, marker))
    }

    /// Build a `make kani-check` invocation against this fixture.
    pub(crate) fn command(&self) -> Command {
        self.command_with_kani_home(&self.kani_home)
    }

    /// Build an invocation naming `home` as the Kani home.
    ///
    /// The home is a parameter rather than always this fixture's own so a case
    /// can name an empty value, which is the one setting the recipe refuses.
    pub(crate) fn command_with_kani_home(&self, home: &Path) -> Command {
        let mut command = self.command_without_kani_home();
        command.arg(format!("KANI_HOME={}", home.display()));
        command
    }

    /// Build an invocation that leaves `KANI_HOME` unset.
    ///
    /// The recipe falls back to `$HOME/.kani` in that case, so a case can blank
    /// `HOME` too and leave the fallback with nothing to resolve.
    pub(crate) fn command_without_kani_home(&self) -> Command {
        let mut command = Command::new("make");
        command
            .arg("--no-print-directory")
            .arg("-f")
            .arg("Makefile")
            .arg(format!("KANI={}", self.executable.display()))
            .arg(format!("KANI_VERSION_FILE={}", self.version_file.display()))
            .env_remove("KANI_HOME");
        command
    }

    /// Run `make kani-check` against this fixture.
    ///
    /// # Errors
    ///
    /// Returns an error if the Make invocation cannot be spawned.
    pub(crate) fn run(&self) -> Result<Output> {
        self.command()
            .arg("kani-check")
            .output()
            .context("run make kani-check")
    }
}

/// Render the shell line that makes the verifier answer as `behaviour` says.
fn response_line(behaviour: Behaviour<'_>) -> String {
    match behaviour {
        Behaviour::Reports(version) => format!("printf 'kani {version}\\n'\n"),
        Behaviour::Fails { status, noise } => {
            format!("printf '%s\\n' '{noise}' >&2\nexit {status}\n")
        }
    }
}

/// Build the fake verifier script `behaviour` describes.
fn script_for(behaviour: Behaviour<'_>, invocation_log: &Path) -> String {
    let mut script = format!(
        "#!/bin/sh\nfor argument do\n    printf '%s\\n' \"$argument\" >> '{}'\ndone\n",
        invocation_log.display()
    );
    script.push_str(&response_line(behaviour));
    script
}

/// Write an executable that records its arguments and answers as described.
///
/// # Errors
///
/// Returns an error if the script cannot be written or marked executable.
fn write_fake_kani(
    executable: &Path,
    invocation_log: &Path,
    behaviour: Behaviour<'_>,
) -> Result<()> {
    write_executable(executable, &script_for(behaviour, invocation_log))
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
