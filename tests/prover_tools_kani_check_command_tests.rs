//! Behavioural tests for the `kani-check` version-drift guard.
//!
//! `make kani-check` reads the pinned version from `KANI_VERSION_FILE`, asks
//! the installed verifier for `--version`, and compares the two. These tests
//! drive the recipe with a fake verifier so every branch is exercised without
//! installing Kani: a match, a mismatch, a missing pin, a pin that cannot be
//! read at all, a failing verifier, an empty Kani home, and the boundary that
//! keeps the query off the network and out of any cache. Every fixture names an
//! installed payload for its pin, because the recipe refuses a home without one;
//! that refusal itself belongs to `prover_tools_kani_check_tests`.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

/// How the fake verifier answers a `--version` request.
#[derive(Clone, Copy)]
enum Behaviour<'a> {
    /// Print `version` and succeed.
    Reports(&'a str),
    /// Print `noise` to standard error and exit with `status`.
    Fails { status: i32, noise: &'a str },
}

/// A fake verifier plus the files the recipe reads.
struct FakeKani {
    temp: TempDir,
    executable: PathBuf,
    invocation_log: PathBuf,
    version_file: PathBuf,
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
    fn new(pinned_version: &str, behaviour: Behaviour<'_>) -> Result<Self> {
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
    fn path(&self, name: &str) -> PathBuf {
        self.temp.path().join(name)
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

    /// Write a recorder that marks its invocation before failing.
    ///
    /// The recorder stands in for a network-capable tool. It appends to a
    /// marker file *before* exiting non-zero, so an attempt is visible even
    /// when the caller ignores the failure.
    ///
    /// # Errors
    ///
    /// Returns an error if the recorder cannot be written or marked executable.
    fn write_recorder(&self) -> Result<(PathBuf, PathBuf)> {
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
    fn command(&self) -> Command {
        self.command_with_kani_home(&self.kani_home)
    }

    /// Build an invocation naming `home` as the Kani home.
    ///
    /// The home is a parameter rather than always this fixture's own so a case
    /// can name an empty value, which is the one setting the recipe refuses.
    fn command_with_kani_home(&self, home: &Path) -> Command {
        let mut command = Command::new("make");
        command
            .arg("--no-print-directory")
            .arg("-f")
            .arg("Makefile")
            .arg(format!("KANI={}", self.executable.display()))
            .arg(format!("KANI_VERSION_FILE={}", self.version_file.display()))
            .arg(format!("KANI_HOME={}", home.display()));
        command
    }

    /// Run `make kani-check` against this fixture.
    ///
    /// # Errors
    ///
    /// Returns an error if the Make invocation cannot be spawned.
    fn run(&self) -> Result<Output> {
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

/// Check that a matching pin succeeds and that the verifier is asked for its
/// version rather than being run as a proof command.
#[test]
fn kani_check_accepts_a_matching_version_and_queries_the_verifier() -> Result<()> {
    let fake = FakeKani::new("0.67.0", Behaviour::Reports("0.67.0"))?;

    let output = fake.run()?;

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
    let fake = FakeKani::new("0.66.0", Behaviour::Reports("0.67.0"))?;

    let output = fake.run()?;

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
    let fake = FakeKani::new("0.67.0", Behaviour::Reports("0.67.0"))?;
    fs::remove_file(&fake.version_file).context("remove fake Kani version file")?;

    let output = fake.run()?;

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
    let fake = FakeKani::new("0.67.0", Behaviour::Reports("0.67.0"))?;
    fs::remove_file(&fake.version_file).context("remove fake Kani version file")?;
    fs::create_dir(&fake.version_file).context("replace the pin with a directory")?;

    let output = fake.run()?;

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

/// Check that a failing verifier yields a category and status without echoing
/// anything the verifier wrote.
///
/// The verifier is an external binary, so its output is untrusted: it could
/// carry a credential, forge a surrounding log line, or flood a terminal.
/// Bounding and escaping such text does not make it safe to print, so the
/// recipe reports the category and exit status instead and leaves the raw
/// output to a hand-run of the verifier.
#[test]
fn kani_check_reports_a_category_without_echoing_verifier_output() -> Result<()> {
    let noise = concat!(
        "kani 9.9.9 token=sk-live-9f8e7d6c5b4a ",
        "\u{1b}[31mforged-line\u{1b}[0m\n",
        "prover-tools: target=kani-check category=version-mismatch\n"
    );
    let fake = FakeKani::new("0.67.0", Behaviour::Fails { status: 9, noise })?;

    let output = fake.run()?;

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
    ensure!(
        !stderr.contains("sk-live-9f8e7d6c5b4a"),
        "the diagnostic must not echo a secret from the verifier, got `{stderr}`"
    );
    ensure!(
        !stderr.contains("forged-line"),
        "the diagnostic must not echo the verifier's own text, got `{stderr}`"
    );
    ensure!(
        stderr.matches("category=version-mismatch").count() == 0,
        "the verifier must not be able to inject a category, got `{stderr}`"
    );
    ensure!(
        !stderr
            .chars()
            .any(|character| character.is_control() && character != '\n'),
        "the diagnostic must carry no control characters beyond line breaks, got `{stderr}`"
    );
    Ok(())
}

/// Check that unparsable verifier output is reported without echoing it.
#[test]
fn kani_check_reports_unparsable_output_without_echoing_it() -> Result<()> {
    let noise = "kani token=sk-live-9f8e7d6c5b4a without a release number";
    let fake = FakeKani::new("0.67.0", Behaviour::Reports(noise))?;

    let output = fake.run()?;

    ensure!(!output.status.success(), "unparsable output should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("category=version-unparsable"),
        "the diagnostic should carry a stable category, got `{stderr}`"
    );
    ensure!(
        !stderr.contains("sk-live-9f8e7d6c5b4a"),
        "the diagnostic must not echo a secret from the verifier, got `{stderr}`"
    );
    Ok(())
}

/// Check that an empty `KANI_HOME` is refused rather than resolved differently.
///
/// `cargo-kani` reads an empty `KANI_HOME` as the current directory, whereas
/// the `${KANI_HOME:-...}` default the recipe would otherwise apply reads it as
/// `$HOME/.kani`. The guard would then approve one home while the launcher
/// installed into another, so the recipe refuses the setting rather than
/// guessing which one was meant.
#[test]
fn kani_check_refuses_an_empty_kani_home() -> Result<()> {
    let fake = FakeKani::new("0.67.0", Behaviour::Reports("0.67.0"))?;

    let output = fake
        .command_with_kani_home(Path::new(""))
        .arg("kani-check")
        .output()
        .context("run make kani-check with an empty Kani home")?;

    ensure!(!output.status.success(), "an empty Kani home should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("category=kani-home-empty"),
        "the diagnostic should carry a stable category, got `{stderr}`"
    );
    ensure!(
        fake.invocation_args()?.is_empty(),
        "the verifier must not run when the Kani home is ambiguous"
    );
    Ok(())
}

/// Check that the query never reaches the network, a cache, or the remote
/// runner, even when such a call would fail silently.
///
/// The recipe has no reason to resolve the shared CLI, fetch anything, or write
/// a cache. A recorder stands in for both network-capable tools the repository
/// defines and marks the attempt *before* failing, so an ignored error is still
/// visible. The cache and home directories start empty and must stay empty,
/// which catches a write that succeeded rather than one that failed.
#[test]
fn kani_check_makes_no_network_or_cache_attempts() -> Result<()> {
    let fake = FakeKani::new("0.67.0", Behaviour::Reports("0.67.0"))?;
    let (recorder, marker) = fake.write_recorder()?;
    let cache = fake.path("cache");
    let home = fake.path("home");
    fs::create_dir_all(&cache).context("create isolated cache directory")?;
    fs::create_dir_all(&home).context("create isolated home directory")?;

    let output = fake
        .command()
        .arg(format!("PROVER_TOOLS={}", recorder.display()))
        .arg(format!("UV={}", recorder.display()))
        .env("UV_CACHE_DIR", &cache)
        .env("XDG_CACHE_HOME", &cache)
        .env("HOME", &home)
        .arg("kani-check")
        .output()
        .context("run make kani-check with a recorder in place of the network tools")?;

    ensure!(
        output.status.success(),
        "the query must succeed without reaching the network: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        !marker.exists(),
        "the query must not attempt to reach the remote runner or a fetcher"
    );
    for directory in [&cache, &home] {
        let entries = fs::read_dir(directory)
            .with_context(|| format!("read {}", directory.display()))?
            .count();
        ensure!(
            entries == 0,
            "the query must not write to {}, found {entries} entries",
            directory.display()
        );
    }
    Ok(())
}
