//! CLI execution and command dispatch logic.
//!
//! This module keeps `main` minimal by providing a single entry point that
//! handles command execution. It now delegates build requests to the Ninja
//! subprocess, streaming its output back to the user.

use crate::cli::{BuildArgs, Cli, Commands};
use crate::{ir::BuildGraph, manifest, ninja_gen};
use anyhow::{Context, Result};
use std::borrow::Cow;
use std::path::Path;
use tempfile::NamedTempFile;
use tracing::{debug, info};

/// Default Ninja executable to invoke.
pub const NINJA_PROGRAM: &str = "ninja";
/// Environment variable override for the Ninja executable.
pub use ninja_env::NINJA_ENV;

mod process;
#[cfg(any(test, doctest))]
pub use process::doc;
pub use process::run_ninja;

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
            process::write_ninja_file(&file, &ninja)?;
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
    let _tmp_file_guard: Option<NamedTempFile>;
    if let Some(path) = &args.emit {
        process::write_ninja_file(path, &ninja)?;
        build_path = Cow::Borrowed(path.as_path());
        _tmp_file_guard = None;
    } else {
        let tmp = process::create_temp_ninja_file(&ninja)?;
        build_path = Cow::Owned(tmp.path().to_path_buf());
        _tmp_file_guard = Some(tmp);
    }

    let program = process::resolve_ninja_program();
    run_ninja(program.as_path(), cli, build_path.as_ref(), &targets).with_context(|| {
        format!(
            "running {} with build file {}",
            program.display(),
            build_path.display()
        )
    })?;
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
        .with_context(|| format!("loading manifest at {}", manifest_path.display()))?;
    if tracing::enabled!(tracing::Level::DEBUG) {
        let ast_json = serde_json::to_string_pretty(&manifest).context("serialising manifest")?;
        debug!("AST:\n{ast_json}");
    }
    let graph = BuildGraph::from_manifest(&manifest).context("building graph")?;
    let ninja = ninja_gen::generate(&graph).context("generating Ninja file")?;
    Ok(NinjaContent::new(ninja))
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
