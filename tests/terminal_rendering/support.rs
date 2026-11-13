//! Own an isolated child workspace and command environment for rendering tests.
//!
//! Every rendering test needs the same three things: a scratch directory the
//! binary can write into, a command line built with the same `clap` definition
//! the binary uses, and the display plan that command line resolves to under
//! the terminal facts the test sets up. [`TerminalWorkspace`] supplies the
//! first, and [`display_plan_for_args`] resolves the third from the second.
//! [`visible_text`] is the shared reader for captured bytes: the stage labels
//! carry Fluent bidi isolates, so a raw substring search silently finds nothing.

use anyhow::{Context, Result};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use clap::Parser;
use netsuke::cli::Cli;
use netsuke::display_plan::{DisplayPlan, TerminalFacts, resolve_display_plan};
use netsuke::locale_resolution::{LocaleEnvProvider, SystemEnv};
use netsuke::output_mode::EnvSignals;
#[cfg(unix)]
use std::time::Duration;
use std::{path::Path, process::Command};
use tempfile::{TempDir, tempdir};
use test_support::ninja_script::write_fake_ninja_script;
#[cfg(unix)]
use test_support::terminal::{Attach, PtyOutput, PtyRequest, run_in_pty};

/// Stage labels emitted by a successful `generate` command in English.
pub const STAGE_LABELS: [&str; 6] = [
    "Stage 1/6: Reading manifest file",
    "Stage 2/6: Parsing YAML document",
    "Stage 3/6: Expanding template directives",
    "Stage 4/6: Deserializing and rendering manifest values",
    "Stage 5/6: Building and validating dependency graph",
    "Stage 6/6: Synthesizing Ninja build plan",
];

/// Resolve the expected display plan from CLI arguments and terminal facts.
///
/// # Examples
///
/// ```ignore
/// let plan = display_plan_for_args(&["--progress", "always"], &[], false, false)?;
/// ```
///
/// # Errors
///
/// Returns an error when the argument list is not a valid Netsuke CLI.
pub(super) fn display_plan_for_args(
    args: &[&str],
    extra_env: &[(&str, &str)],
    stdout_is_terminal: bool,
    stderr_is_terminal: bool,
) -> Result<DisplayPlan> {
    let cli = Cli::try_parse_from(std::iter::once("netsuke").chain(args.iter().copied()))
        .context("parse display-policy arguments")?;
    let extra_environment = ExtraLocaleEnv(extra_env);
    let signals = EnvSignals::read(|key| {
        extra_environment
            .var(key)
            .or_else(|| (key == "TERM").then(|| String::from("xterm-256color")))
    });
    let facts = TerminalFacts::from_signals(signals, stdout_is_terminal, stderr_is_terminal);
    let os_args = args.iter().map(|arg| (*arg).into()).collect::<Vec<_>>();
    let mut policies = cli.display_policies();
    policies.colour =
        netsuke::locale_resolution::resolve_startup_colour(&os_args, &extra_environment);
    Ok(resolve_display_plan(policies, facts))
}

struct ExtraLocaleEnv<'a>(&'a [(&'a str, &'a str)]);

impl LocaleEnvProvider for ExtraLocaleEnv<'_> {
    fn var(&self, key: &str) -> Option<String> {
        self.0
            .iter()
            .find_map(|(name, value)| (*name == key).then(|| String::from(*value)))
    }
}

/// Keep a temporary Netsukefile workspace and a deterministic fake Ninja alive.
///
/// This fixture belongs to the terminal-rendering integration-test binary.
/// Its command builders clear inherited variables, install an isolated home
/// and locale, and pass only each test's explicit environment rows.
pub struct TerminalWorkspace {
    /// Temporary directory holding the staged manifest and configuration.
    root: TempDir,
    /// Temporary directory holding the fake Ninja executable.
    ninja_directory: TempDir,
    /// Absolute path passed to Netsuke as its Ninja override.
    ninja_path: std::path::PathBuf,
}

impl TerminalWorkspace {
    /// Create a workspace with the repository's minimal manifest and a fake Ninja.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let workspace = TerminalWorkspace::new()?;
    /// let output = workspace.run_piped(&["--progress", "always", "generate"], &[])?;
    /// assert!(output.status.success());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if a temporary directory, capability handle, manifest
    /// copy or fake Ninja script cannot be created.
    pub fn new() -> Result<Self> {
        let root = tempdir().context("create terminal-rendering workspace")?;
        let root_path = Utf8Path::from_path(root.path())
            .context("terminal-rendering workspace path is not UTF-8")?;
        let workspace = Dir::open_ambient_dir(root_path, ambient_authority())
            .context("open terminal-rendering workspace")?;
        let repository = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
            .context("open repository for manifest staging")?;
        repository
            .copy("tests/data/minimal.yml", &workspace, "Netsukefile")
            .context("stage the minimal Netsukefile")?;

        let ninja_directory = tempdir().context("create fake Ninja directory")?;
        let ninja_path = Utf8Path::from_path(ninja_directory.path())
            .context("fake Ninja directory path is not UTF-8")?;
        let capability_dir = Dir::open_ambient_dir(ninja_path, ambient_authority())
            .context("open fake Ninja directory")?;
        let script_name = fake_ninja_name();
        write_fake_ninja_script(
            &capability_dir,
            Utf8Path::new(&script_name),
            &["[1/2] cc -c src/a.c", "[2/2] cc -c src/b.c"],
            None,
        )
        .context("write fake Ninja task-status script")?;
        let executable_path = ninja_directory.path().join(script_name);

        Ok(Self {
            root,
            ninja_directory,
            ninja_path: executable_path,
        })
    }

    /// Return the temporary root used for files supplied to the child process.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let workspace = TerminalWorkspace::new()?;
    /// test_support::fs::write(workspace.root_path().join("broken.toml"), "{")?;
    /// ```
    #[must_use]
    pub fn root_path(&self) -> &Path {
        self.root.path()
    }

    /// Run Netsuke with piped stdout and stderr and an allowlisted environment.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let output = workspace.run_piped(&["--accessibility", "on", "generate"], &[])?;
    /// assert!(output.status.success());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if Netsuke cannot be spawned or its output cannot be
    /// collected.
    pub fn run_piped(
        &self,
        args: &[&str],
        extra_env: &[(&str, &str)],
    ) -> Result<std::process::Output> {
        let mut command = assert_cmd::Command::from_std(self.netsuke_command(args, extra_env));
        command.output().context("run piped Netsuke command")
    }

    /// Run Netsuke with one selected output stream on a Unix pseudo-terminal.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let output = workspace.run_pty(
    ///     &["--progress", "always", "generate"],
    ///     &[("TERM", "xterm-256color")],
    ///     Attach::StderrOnly,
    /// )?;
    /// assert!(output.status.success());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the pseudo-terminal or child process cannot be
    /// prepared, the command fails to exit within its bound, or its output
    /// readers cannot be joined.
    #[cfg(unix)]
    pub fn run_pty(
        &self,
        args: &[&str],
        extra_env: &[(&str, &str)],
        attach: Attach,
    ) -> Result<PtyOutput> {
        let command = self.netsuke_command(args, extra_env);
        run_in_pty(PtyRequest {
            command,
            attach,
            timeout: Duration::from_secs(30),
        })
    }

    /// Build a child command with only the terminal test's allowlisted environment.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let command = workspace.netsuke_command(&["generate"], &[]);
    /// ```
    fn netsuke_command(&self, args: &[&str], extra_env: &[(&str, &str)]) -> Command {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin!("netsuke"));
        command
            .current_dir(self.root.path())
            .env_clear()
            .env("PATH", self.ninja_directory.path())
            .env("HOME", self.root.path())
            .env("TMPDIR", self.root.path())
            .env("LC_ALL", "C.UTF-8")
            .env("NETSUKE_LOCALE", "en-US")
            .env("TERM", "xterm-256color")
            .env(netsuke::runner::NINJA_ENV, &self.ninja_path)
            .args(args);
        if cfg!(windows)
            && let Some(system_root) = SystemEnv.var("SystemRoot")
        {
            command.env("SystemRoot", system_root);
        }
        for &(key, value) in extra_env {
            command.env(key, value);
        }
        command
    }
}

/// Create a terminal workspace whose staged manifest has been removed.
///
/// Removing the manifest is what makes a run fail during manifest resolution,
/// which is the only way to reach the Miette diagnostic path from a test.
///
/// # Examples
///
/// ```ignore
/// let workspace = empty_workspace()?;
/// let output = workspace.run_piped(&["--color", "never", "generate"], &[])?;
/// assert!(!output.status.success());
/// ```
///
/// # Errors
///
/// Returns an error if the workspace cannot be created or the staged manifest
/// cannot be removed.
pub(super) fn empty_workspace() -> Result<TerminalWorkspace> {
    let workspace = TerminalWorkspace::new()?;
    test_support::fs::remove_file(workspace.root_path().join("Netsukefile"))
        .context("remove the staged manifest")?;
    Ok(workspace)
}

/// Return the executable name for the current platform.
///
/// # Examples
///
/// ```ignore
/// let name = fake_ninja_name();
/// assert!(name == "ninja" || name == "ninja.cmd");
/// ```
fn fake_ninja_name() -> &'static str {
    if cfg!(windows) { "ninja.cmd" } else { "ninja" }
}

/// Return stage labels in the order they first appear in captured output.
///
/// # Examples
///
/// ```ignore
/// use tests::terminal_rendering::support::{STAGE_LABELS, stage_labels_in_output_order};
/// let output = format!("{} then {}", STAGE_LABELS[1], STAGE_LABELS[0]);
/// assert_eq!(stage_labels_in_output_order(&output), [STAGE_LABELS[1], STAGE_LABELS[0]]);
/// ```
#[must_use]
pub fn stage_labels_in_output_order(output: &str) -> Vec<&'static str> {
    let mut labels = STAGE_LABELS
        .iter()
        .filter_map(|label| output.find(label).map(|position| (position, *label)))
        .collect::<Vec<_>>();
    labels.sort_unstable_by_key(|(position, _)| *position);
    labels.into_iter().map(|(_, label)| label).collect()
}

/// Decode captured terminal bytes and remove Fluent bidirectional isolates.
///
/// Use this only for human-text assertions; pass the original bytes to the
/// terminal classifier so control and accessibility markers remain observable.
///
/// # Examples
///
/// ```
/// assert_eq!(visible_text("ready".as_bytes()), "ready");
/// ```
#[must_use]
pub fn visible_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    test_support::fluent::normalize_fluent_isolates(&text)
}
