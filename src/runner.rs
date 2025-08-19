//! CLI execution and command dispatch logic.
//!
//! This module keeps `main` minimal by providing a single entry point that
//! handles command execution. It now delegates build requests to the Ninja
//! subprocess, streaming its output back to the user.

use crate::cli::{BuildArgs, Cli, Commands};
use crate::{ir::BuildGraph, manifest, ninja_gen};
use miette::{Context, IntoDiagnostic, Result};
use serde_json;
use std::borrow::Cow;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use tempfile::{Builder, NamedTempFile};
use tracing::{debug, info};

/// Default Ninja executable to invoke.
pub const NINJA_PROGRAM: &str = "ninja";
/// Environment variable override for the Ninja executable.
pub use ninja_env::NINJA_ENV;

// Public helpers for doctests only. This exposes internal helpers as a stable
// testing surface without exporting them in release builds.
#[cfg(doctest)]
#[doc(hidden)]
pub mod doc {
    pub use super::CommandArg;

    // Public wrappers to expose crate-private helpers to doctests.
    #[must_use]
    pub fn contains_sensitive_keyword(arg: &CommandArg) -> bool {
        super::contains_sensitive_keyword(arg)
    }
    #[must_use]
    pub fn is_sensitive_arg(arg: &CommandArg) -> bool {
        super::is_sensitive_arg(arg)
    }
    #[must_use]
    pub fn redact_argument(arg: &CommandArg) -> CommandArg {
        super::redact_argument(arg)
    }
    #[must_use]
    pub fn redact_sensitive_args(args: &[CommandArg]) -> Vec<CommandArg> {
        super::redact_sensitive_args(args)
    }
}

#[derive(Debug, Clone)]
pub struct NinjaContent(String);
impl NinjaContent {
    #[must_use]
    pub fn new(content: String) -> Self {
        Self(content)
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct CommandArg(String);
impl CommandArg {
    #[must_use]
    pub fn new(arg: String) -> Self {
        Self(arg)
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Target list passed through to Ninja.
/// An empty slice means “use the defaults” emitted by IR generation
/// (default targets).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildTargets<'a>(&'a [String]);
impl<'a> BuildTargets<'a> {
    #[must_use]
    pub fn new(targets: &'a [String]) -> Self {
        Self(targets)
    }
    #[must_use]
    pub fn as_slice(&self) -> &'a [String] {
        self.0
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[expect(
    clippy::derivable_impls,
    reason = "Default derive requires 'static lifetime; manual impl returns empty slice."
)]
impl Default for BuildTargets<'_> {
    fn default() -> Self {
        Self(&[])
    }
}

/// Execute the parsed [`Cli`] commands.
///
/// # Errors
///
/// Returns an error if manifest generation or the Ninja process fails.
pub fn run(cli: &Cli) -> Result<()> {
    let command = cli.command.clone().unwrap_or(Commands::Build(BuildArgs {
        emit: None,
        targets: Vec::new(),
    }));
    match command {
        Commands::Build(args) => handle_build(cli, &args),
        Commands::Manifest { file } => {
            let ninja = generate_ninja(cli)?;
            write_ninja_file(&file, &ninja)?;
            Ok(())
        }
        Commands::Clean => {
            info!(target: "netsuke::subcommand", subcommand = "clean", "Subcommand requested");
            Ok(())
        }
        Commands::Graph => {
            info!(target: "netsuke::subcommand", subcommand = "graph", "Subcommand requested");
            Ok(())
        }
    }
}

/// Resolve the manifest, generate the Ninja file and invoke the build.
///
/// # Errors
///
/// Returns an error if manifest generation or Ninja execution fails.
///
/// # Examples
/// ```ignore
/// use netsuke::cli::{BuildArgs, Cli};
/// use netsuke::runner::handle_build;
/// let cli = Cli { file: "Netsukefile".into(), directory: None, jobs: None, verbose: false, command: None };
/// let args = BuildArgs { emit: None, targets: vec![] };
/// handle_build(&cli, &args).unwrap();
/// ```
fn handle_build(cli: &Cli, args: &BuildArgs) -> Result<()> {
    let ninja = generate_ninja(cli)?;
    let targets = BuildTargets::new(&args.targets);

    // Normalize the build file path and keep the temporary file alive for the
    // duration of the Ninja invocation. Borrow the emitted path when provided
    // to avoid unnecessary allocation.
    let build_path: Cow<Path>;
    let mut tmp_file: Option<NamedTempFile> = None;
    if let Some(path) = &args.emit {
        write_ninja_file(path, &ninja)?;
        build_path = Cow::Borrowed(path.as_path());
    } else {
        let tmp = create_temp_ninja_file(&ninja)?;
        tmp_file = Some(tmp);
        build_path = Cow::Borrowed(
            tmp_file
                .as_ref()
                .expect("temporary Ninja file should exist")
                .path(),
        );
    }

    let program = resolve_ninja_program();
    run_ninja(program.as_path(), cli, build_path.as_ref(), &targets)
        .into_diagnostic()
        .wrap_err("run ninja")?;
    drop(tmp_file);
    Ok(())
}

/// Create a temporary Ninja file on disk containing `content`.
///
/// # Errors
///
/// Returns an error if the file cannot be created or written.
///
/// # Examples
/// ```ignore
/// use netsuke::runner::{create_temp_ninja_file, NinjaContent};
/// let tmp = create_temp_ninja_file(&NinjaContent::new("".into())).unwrap();
/// assert!(tmp.path().to_string_lossy().ends_with(".ninja"));
/// ```
fn create_temp_ninja_file(content: &NinjaContent) -> Result<NamedTempFile> {
    let tmp = Builder::new()
        .prefix("netsuke.")
        .suffix(".ninja")
        .tempfile()
        .into_diagnostic()
        .wrap_err("create temp file")?;
    write_ninja_file(tmp.path(), content)?;
    Ok(tmp)
}

/// Write `content` to `path` and log the file's location.
///
/// # Errors
///
/// Returns an error if the file cannot be written.
///
/// # Examples
/// ```ignore
/// let content = NinjaContent::new("rule cc\n".to_string());
/// write_ninja_file(Path::new("out.ninja"), &content).unwrap();
/// ```
fn write_ninja_file(path: &Path, content: &NinjaContent) -> Result<()> {
    // Ensure the parent directory exists; guard against empty components so we
    // do not attempt to create the current directory on some platforms.
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to create parent directory {}", parent.display()))?;
    }
    fs::write(path, content.as_str())
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to write Ninja file to {}", path.display()))?;
    info!("Generated Ninja file at {}", path.display());
    Ok(())
}

/// Generate the Ninja manifest string from the Netsuke manifest referenced by `cli`.
///
/// # Errors
///
/// Returns an error if the manifest cannot be loaded or translated.
///
/// # Examples
/// ```ignore
/// use netsuke::cli::{Cli, Commands};
/// use netsuke::runner::generate_ninja;
/// let cli = Cli {
///     file: "Netsukefile".into(),
///     directory: None,
///     jobs: None,
///     verbose: false,
///     command: Some(Commands::Build(BuildArgs { emit: None, targets: vec![] })),
/// };
/// let ninja = generate_ninja(&cli).expect("generate");
/// assert!(ninja.as_str().contains("rule"));
/// ```
fn generate_ninja(cli: &Cli) -> Result<NinjaContent> {
    let manifest_path = resolve_manifest_path(cli);
    let manifest = manifest::from_path(&manifest_path)
        .wrap_err_with(|| format!("loading manifest at {}", manifest_path.display()))?;
    let ast_json = serde_json::to_string_pretty(&manifest)
        .into_diagnostic()
        .wrap_err("serialising manifest")?;
    debug!("AST:\n{ast_json}");
    let graph = BuildGraph::from_manifest(&manifest)
        .into_diagnostic()
        .wrap_err("building graph")?;
    Ok(NinjaContent::new(ninja_gen::generate(&graph)))
}

/// Determine the manifest path respecting the CLI's directory option.
///
/// # Examples
/// ```ignore
/// use crate::cli::Cli;
/// use crate::runner::resolve_manifest_path;
/// let cli = Cli { file: "Netsukefile".into(), directory: None, jobs: None, verbose: false, command: None };
/// assert!(resolve_manifest_path(&cli).ends_with("Netsukefile"));
/// ```
#[must_use]
fn resolve_manifest_path(cli: &Cli) -> std::path::PathBuf {
    cli.directory
        .as_ref()
        .map_or_else(|| cli.file.clone(), |dir| dir.join(&cli.file))
}

/// Determine which Ninja executable to invoke.
#[must_use]
fn resolve_ninja_program() -> PathBuf {
    std::env::var_os(NINJA_ENV).map_or_else(|| PathBuf::from(NINJA_PROGRAM), PathBuf::from)
}

/// Check if `arg` contains a sensitive keyword.
///
/// # Examples
/// ```
/// # use netsuke::runner::doc::{CommandArg, contains_sensitive_keyword};
/// assert!(contains_sensitive_keyword(&CommandArg::new("token=abc".into())));
/// assert!(!contains_sensitive_keyword(&CommandArg::new("path=/tmp".into())));
/// ```
pub(crate) fn contains_sensitive_keyword(arg: &CommandArg) -> bool {
    let lower = arg.as_str().to_lowercase();
    lower.contains("password") || lower.contains("token") || lower.contains("secret")
}

/// Determine whether the argument should be redacted.
///
/// # Examples
/// ```
/// # use netsuke::runner::doc::{CommandArg, is_sensitive_arg};
/// assert!(is_sensitive_arg(&CommandArg::new("password=123".into())));
/// assert!(!is_sensitive_arg(&CommandArg::new("file=readme".into())));
/// ```
pub(crate) fn is_sensitive_arg(arg: &CommandArg) -> bool {
    contains_sensitive_keyword(arg)
}

/// Redact sensitive information in a single argument.
///
/// Sensitive values are replaced with `***REDACTED***`, preserving keys.
///
/// # Examples
/// ```
/// # use netsuke::runner::doc::{CommandArg, redact_argument};
/// let arg = CommandArg::new("token=abc".into());
/// assert_eq!(redact_argument(&arg).as_str(), "token=***REDACTED***");
/// let arg = CommandArg::new("path=/tmp".into());
/// assert_eq!(redact_argument(&arg).as_str(), "path=/tmp");
/// ```
pub(crate) fn redact_argument(arg: &CommandArg) -> CommandArg {
    if is_sensitive_arg(arg) {
        let redacted = arg.as_str().split_once('=').map_or_else(
            || "***REDACTED***".to_string(),
            |(key, _)| format!("{key}=***REDACTED***"),
        );
        CommandArg::new(redacted)
    } else {
        arg.clone()
    }
}

/// Redact sensitive information from all `args`.
///
/// # Examples
/// ```
/// # use netsuke::runner::doc::{CommandArg, redact_sensitive_args};
/// let args = vec![
///     CommandArg::new("ninja".into()),
///     CommandArg::new("token=abc".into()),
/// ];
/// let redacted = redact_sensitive_args(&args);
/// assert_eq!(redacted[1].as_str(), "token=***REDACTED***");
/// ```
pub(crate) fn redact_sensitive_args(args: &[CommandArg]) -> Vec<CommandArg> {
    args.iter().map(redact_argument).collect()
}

/// Invoke the Ninja executable with the provided CLI settings.
///
/// The function forwards the job count and working directory to Ninja,
/// specifies the temporary build file, and streams its standard output and
/// error back to the user.
///
/// # Errors
///
/// Returns an [`io::Error`] if the Ninja process fails to spawn or reports a
/// non-zero exit status.
///
/// # Panics
///
/// Panics if the child's output streams cannot be captured.
pub fn run_ninja(
    program: &Path,
    cli: &Cli,
    build_file: &Path,
    targets: &BuildTargets<'_>,
) -> io::Result<()> {
    let mut cmd = Command::new(program);
    if let Some(dir) = &cli.directory {
        // Resolve and canonicalise the directory so Ninja receives a stable
        // absolute path. Using only `current_dir` avoids combining it with
        // Ninja's own `-C` flag which would otherwise double-apply the
        // directory and break relative paths.
        let dir = fs::canonicalize(dir)?;
        cmd.current_dir(dir);
    }
    if let Some(jobs) = cli.jobs {
        cmd.arg("-j").arg(jobs.to_string());
    }
    // Canonicalise the build file path so Ninja resolves it correctly from the
    // working directory. Fall back to the original on failure so Ninja can
    // surface a meaningful error.
    let build_file_path = build_file
        .canonicalize()
        .unwrap_or_else(|_| build_file.to_path_buf());
    cmd.arg("-f").arg(&build_file_path);
    cmd.args(targets.as_slice());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let program = cmd.get_program().to_string_lossy().into_owned();
    let args: Vec<CommandArg> = cmd
        .get_args()
        .map(|a| CommandArg::new(a.to_string_lossy().into_owned()))
        .collect();
    let redacted_args = redact_sensitive_args(&args);
    let arg_strings: Vec<&str> = redacted_args.iter().map(CommandArg::as_str).collect();
    info!("Running command: {} {}", program, arg_strings.join(" "));

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().expect("child stdout");
    let stderr = child.stderr.take().expect("child stderr");

    let out_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut handle = io::stdout();
        for line in reader.lines().map_while(Result::ok) {
            let _ = writeln!(handle, "{line}");
        }
    });
    let err_handle = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut handle = io::stderr();
        for line in reader.lines().map_while(Result::ok) {
            let _ = writeln!(handle, "{line}");
        }
    });

    let status = child.wait()?;
    let _ = out_handle.join();
    let _ = err_handle.join();

    if status.success() {
        Ok(())
    } else {
        #[expect(
            clippy::io_other_error,
            reason = "use explicit error kind for compatibility with older Rust"
        )]
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("ninja exited with {status}"),
        ))
    }
}
