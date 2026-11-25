//! Isolated command adapters and assertions for the release-candidate installer.
//!
//! The harness runs the production `install.sh` against fake `git` and `cargo`
//! executables, so each test controls the resolved identity and observes the
//! exact commands the installer issued.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::{Command, Output},
};

use anyhow::{Context, Result, ensure};
use tempfile::TempDir;
use test_support::{fs as test_fs, write_exec_with_content};

pub(crate) const CANDIDATE_REVISION: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678";
/// Stand in for Git, recording each invocation.
const FAKE_GIT_SCRIPT: &str = include_str!("../data/release_candidate_installer/fake_git.sh");
/// Stand in for Cargo, resolving its output directory as Cargo does.
const FAKE_CARGO_SCRIPT: &str = include_str!("../data/release_candidate_installer/fake_cargo.sh");
pub(crate) const CANDIDATE_VERSION: &str = "0.1.0-beta2";
const TEST_PATH: &str = "/usr/bin:/bin";
const TEST_TOKEN: &str = "installer-test-token";
const INSTALLER_OPERATIONS: [&str; 5] = [
    "candidate_revision_validation",
    "git_fetch",
    "candidate_revision_verification",
    "locked_cargo_build",
    "candidate_version_check",
];

/// Represent controlled installer inputs for one harness invocation.
pub(crate) struct InstallerRun<'a> {
    pub(crate) runner_os: &'a str,
    pub(crate) candidate_revision: &'a str,
    pub(crate) resolved_revision: &'a str,
    pub(crate) version: &'a str,
}

pub(crate) struct InstallerHarness {
    root: TempDir,
    bash_env_path: PathBuf,
    fake_bin_dir: PathBuf,
    pub(crate) git_args_path: PathBuf,
    pub(crate) cargo_args_path: PathBuf,
    cargo_rustflags_path: PathBuf,
    github_output_path: PathBuf,
}

impl InstallerHarness {
    /// Create an isolated installer harness.
    pub(crate) fn new() -> Result<Self> {
        let root = TempDir::new().context("create installer test directory")?;
        let fake_bin_dir = root.path().join("fake-bin");
        test_fs::create_dir(&fake_bin_dir).context("create fake command directory")?;
        let cargo_args_path = root.path().join("cargo-args");
        let cargo_rustflags_path = root.path().join("cargo-rustflags");
        let git_args_path = root.path().join("git-args");
        let github_output_path = root.path().join("github-output");
        let bash_env_path = root.path().join("bash-env");
        test_fs::write(&bash_env_path, "").context("write empty Bash environment")?;

        write_exec_with_content(&fake_bin_dir, "git", FAKE_GIT_SCRIPT)?;
        write_exec_with_content(&fake_bin_dir, "cargo", FAKE_CARGO_SCRIPT)?;

        Ok(Self {
            root,
            bash_env_path,
            fake_bin_dir,
            git_args_path,
            cargo_args_path,
            cargo_rustflags_path,
            github_output_path,
        })
    }

    /// Run the installer with controlled identity and platform inputs.
    pub(crate) fn run(
        &self,
        runner_os: &str,
        resolved_revision: &str,
        version: &str,
    ) -> Result<Output> {
        self.run_with_candidate_revision(&InstallerRun {
            runner_os,
            candidate_revision: CANDIDATE_REVISION,
            resolved_revision,
            version,
        })
    }

    /// Run the installer with a caller-supplied candidate revision.
    pub(crate) fn run_with_candidate_revision(&self, inputs: &InstallerRun<'_>) -> Result<Output> {
        self.command(inputs)
            .output()
            .context("run release-candidate installer")
    }

    /// Run a valid Linux installation that inherits the given environment.
    pub(crate) fn run_inheriting(&self, inherited: &[(&str, &str)]) -> Result<Output> {
        let mut command = self.command(&InstallerRun {
            runner_os: "Linux",
            candidate_revision: CANDIDATE_REVISION,
            resolved_revision: CANDIDATE_REVISION,
            version: CANDIDATE_VERSION,
        });
        command.envs(inherited.iter().copied());
        command.output().context("run release-candidate installer")
    }

    /// Build the installer command with no inherited Cargo build settings.
    ///
    /// The gate targets export `RUSTFLAGS`, and a developer may export an
    /// output directory, so each is cleared to keep the installer's own
    /// handling observable.
    fn command(&self, inputs: &InstallerRun<'_>) -> Command {
        let mut command = Command::new("bash");
        command
            .arg(installer_script())
            .env("BASH_ENV", &self.bash_env_path)
            .env("GITHUB_OUTPUT", &self.github_output_path)
            .env("NETSUKE_CANDIDATE_REVISION", inputs.candidate_revision)
            .env("NETSUKE_CANDIDATE_VERSION", CANDIDATE_VERSION)
            .env("NETSUKE_CARGO_ARGS", &self.cargo_args_path)
            .env("NETSUKE_CARGO_RUSTFLAGS", &self.cargo_rustflags_path)
            .env("NETSUKE_GIT_ARGS", &self.git_args_path)
            .env("GH_TOKEN", TEST_TOKEN)
            .env("NETSUKE_FAKE_BINARY_NAME", binary_name(inputs.runner_os))
            .env("NETSUKE_FAKE_RESOLVED_REVISION", inputs.resolved_revision)
            .env("NETSUKE_FAKE_VERSION", inputs.version)
            .env(
                "PATH",
                format!("{}:{TEST_PATH}", self.fake_bin_dir.display()),
            )
            .env("RUNNER_OS", inputs.runner_os)
            .env("RUNNER_TEMP", self.root.path())
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_TARGET_DIR")
            .env_remove("CARGO_BUILD_TARGET");
        command
    }

    /// Read installer outputs emitted through `GITHUB_OUTPUT`.
    pub(crate) fn outputs(&self) -> Result<BTreeMap<String, String>> {
        let contents = test_fs::read_to_string(&self.github_output_path)
            .context("read installer GitHub output")?;
        Ok(contents
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect())
    }

    /// Read the `RUSTFLAGS` value Cargo received, or `<unset>`.
    pub(crate) fn cargo_rustflags(&self) -> Result<String> {
        test_fs::read_to_string(&self.cargo_rustflags_path).context("read recorded cargo RUSTFLAGS")
    }

    /// Read recorded Cargo build arguments.
    pub(crate) fn cargo_args(&self) -> Result<String> {
        test_fs::read_to_string(&self.cargo_args_path).context("read recorded cargo arguments")
    }

    /// Read recorded Git command arguments.
    pub(crate) fn git_args(&self) -> Result<String> {
        test_fs::read_to_string(&self.git_args_path).context("read recorded Git arguments")
    }
}

/// Locate the production installer script.
fn installer_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".github/actions/install-release-candidate/install.sh")
}

/// Return the platform-specific candidate binary name.
pub(crate) fn binary_name(runner_os: &str) -> &'static str {
    if runner_os == "Windows" {
        "netsuke.exe"
    } else {
        "netsuke"
    }
}

/// Require successful installer events to cover every bounded operation.
pub(crate) fn require_successful_events(output: &Output) -> Result<()> {
    let stderr = String::from_utf8_lossy(&output.stderr);
    for operation in INSTALLER_OPERATIONS {
        ensure!(
            stderr.contains(&format!(
                "release_candidate operation={operation} outcome=started"
            )) && stderr.contains(&format!(
                "release_candidate operation={operation} outcome=success"
            )),
            "installer should emit started and success events for {operation}"
        );
    }
    ensure!(
        !stderr.contains(TEST_TOKEN),
        "installer events should not expose the test token"
    );

    Ok(())
}

/// Require a controlled installer failure to retain its fixed event category.
pub(crate) fn require_failure_event(
    output: &Output,
    operation: &str,
    error_category: &str,
) -> Result<()> {
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains(&format!(
            "release_candidate operation={operation} outcome=failure error_category={error_category}"
        )),
        "installer should emit a fixed failure event for {operation}"
    );
    ensure!(
        !stderr.contains(TEST_TOKEN),
        "installer events should not expose the test token"
    );

    Ok(())
}
