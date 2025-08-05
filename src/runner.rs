//! CLI execution and command dispatch logic.
//!
//! This module keeps [`main`] minimal by providing a single entry point that
//! handles command execution. It now delegates build requests to the Ninja
//! subprocess, streaming its output back to the user.

use crate::cli::{Cli, Commands};
use crate::{ir::BuildGraph, manifest, ninja_gen};
use anyhow::{Context, Result};
use serde_json;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use tempfile::Builder;
use tracing::{debug, info};

/// Execute the parsed [`Cli`] commands.
///
/// # Errors
///
/// Returns an error if manifest generation or the Ninja process fails.
pub fn run(cli: &Cli) -> Result<()> {
    let command = cli.command.clone().unwrap_or(Commands::Build {
        emit: None,
        targets: Vec::new(),
    });
    match command {
        Commands::Build { targets, emit } => {
            let ninja = generate_ninja(cli)?;
            if let Some(path) = emit {
                write_and_log(&path, &ninja)?;
                run_ninja(Path::new("ninja"), cli, &path, &targets)?;
            } else {
                let tmp = Builder::new()
                    .prefix("netsuke.")
                    .suffix(".ninja")
                    .tempfile()
                    .context("create temp file")?;
                write_and_log(tmp.path(), &ninja)?;
                run_ninja(Path::new("ninja"), cli, tmp.path(), &targets)?;
            }
            Ok(())
        }
        Commands::Emit { file } => {
            let ninja = generate_ninja(cli)?;
            write_and_log(&file, &ninja)?;
            Ok(())
        }
        Commands::Clean => {
            println!("Clean requested");
            Ok(())
        }
        Commands::Graph => {
            println!("Graph requested");
            Ok(())
        }
    }
}

/// Write `content` to `path` and log the file's location.
///
/// # Errors
///
/// Returns an [`io::Error`] if the file cannot be written.
///
/// # Examples
/// ```ignore
/// write_and_log(Path::new("out.ninja"), "rule cc\n").unwrap();
/// ```
fn write_and_log(path: &Path, content: &str) -> io::Result<()> {
    fs::write(path, content)?;
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
///     command: Some(Commands::Build { emit: None, targets: vec![] }),
/// };
/// let ninja = generate_ninja(&cli).expect("generate");
/// assert!(ninja.contains("rule"));
/// ```
fn generate_ninja(cli: &Cli) -> Result<String> {
    let manifest_path = cli
        .directory
        .as_ref()
        .map_or_else(|| cli.file.clone(), |dir| dir.join(&cli.file));
    let manifest = manifest::from_path(&manifest_path)
        .with_context(|| format!("loading manifest at {}", manifest_path.display()))?;
    let ast_json = serde_json::to_string_pretty(&manifest).context("serialising manifest")?;
    debug!("AST:\n{ast_json}");
    let graph = BuildGraph::from_manifest(&manifest).context("building graph")?;
    Ok(ninja_gen::generate(&graph))
}

/// Check if `arg` contains a sensitive keyword.
///
/// # Examples
/// ```
/// assert!(contains_sensitive_keyword("token=abc"));
/// assert!(!contains_sensitive_keyword("path=/tmp"));
/// ```
fn contains_sensitive_keyword(arg: &str) -> bool {
    let lower = arg.to_lowercase();
    lower.contains("password") || lower.contains("token") || lower.contains("secret")
}

/// Determine whether the argument should be redacted.
///
/// # Examples
/// ```
/// assert!(is_sensitive_arg("password=123"));
/// assert!(!is_sensitive_arg("file=readme"));
/// ```
fn is_sensitive_arg(arg: &str) -> bool {
    contains_sensitive_keyword(arg)
}

/// Redact sensitive information in a single argument.
///
/// Sensitive values are replaced with `***REDACTED***`, preserving keys.
///
/// # Examples
/// ```
/// assert_eq!(redact_argument("token=abc"), "token=***REDACTED***");
/// assert_eq!(redact_argument("path=/tmp"), "path=/tmp");
/// ```
fn redact_argument(arg: &str) -> String {
    if is_sensitive_arg(arg) {
        arg.split_once('=').map_or_else(
            || "***REDACTED***".to_string(),
            |(key, _)| format!("{key}=***REDACTED***"),
        )
    } else {
        arg.to_string()
    }
}

/// Redact sensitive information from all `args`.
///
/// # Examples
/// ```
/// let args = vec!["ninja".into(), "token=abc".into()];
/// let redacted = redact_sensitive_args(&args);
/// assert_eq!(redacted[1], "token=***REDACTED***");
/// ```
fn redact_sensitive_args(args: &[String]) -> Vec<String> {
    args.iter().map(|arg| redact_argument(arg)).collect()
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
    targets: &[String],
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
    cmd.args(targets);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let program = cmd.get_program().to_string_lossy().into_owned();
    let args: Vec<String> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let redacted_args = redact_sensitive_args(&args);
    info!("Running command: {} {}", program, redacted_args.join(" "));

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
