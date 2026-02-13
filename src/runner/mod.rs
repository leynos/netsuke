//! CLI execution and command dispatch logic.
//!
//! This module keeps `main` minimal by providing a single entry point that
//! handles command execution. It now delegates build requests to the Ninja
//! subprocess, streaming its output back to the user.

mod error;

pub use error::RunnerError;

use crate::cli::{BuildArgs, Cli, Commands};
use crate::localization::{self, keys};
use crate::output_mode::{self, OutputMode};
use crate::status::{
    AccessibleReporter, IndicatifReporter, LocalizationKey, PipelineStage, SilentReporter,
    StatusReporter, report_pipeline_stage,
};
use crate::{ir::BuildGraph, manifest, ninja_gen};
use anyhow::{Context, Result, anyhow};
use camino::Utf8PathBuf;
use std::borrow::Cow;
use std::path::Path;
use tempfile::NamedTempFile;
use tracing::{debug, info};

/// Default Ninja executable to invoke.
pub const NINJA_PROGRAM: &str = "ninja";
/// Environment variable override for the Ninja executable.
pub use ninja_env::NINJA_ENV;

mod process;
#[cfg(doctest)]
pub use process::doc;
pub use process::{run_ninja, run_ninja_tool};

/// Wrapper around generated Ninja manifest text.
#[derive(Debug, Clone)]
pub struct NinjaContent(String);
impl NinjaContent {
    /// Store the provided Ninja manifest string.
    #[must_use]
    pub const fn new(content: String) -> Self {
        Self(content)
    }
    /// Borrow the underlying manifest text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// Consume the wrapper returning the owned manifest string.
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
    /// Wrap a borrowed list of command-line target names.
    #[must_use]
    pub const fn new(targets: &'a [String]) -> Self {
        Self(targets)
    }
    /// Return the underlying slice of target names.
    #[must_use]
    pub const fn as_slice(&self) -> &'a [String] {
        self.0
    }
    /// Indicate whether no explicit targets were provided.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
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
    let mode = output_mode::resolve(cli.accessible);
    let reporter: Box<dyn StatusReporter> = match (mode, cli.progress.unwrap_or(true)) {
        (OutputMode::Accessible, _) => Box::new(AccessibleReporter),
        (OutputMode::Standard, true) => Box::new(IndicatifReporter::new()),
        (OutputMode::Standard, false) => Box::new(SilentReporter),
    };

    let command = cli.command.clone().unwrap_or(Commands::Build(BuildArgs {
        emit: None,
        targets: Vec::new(),
    }));
    match command {
        Commands::Build(args) => handle_build(cli, &args, reporter.as_ref()),
        Commands::Manifest { file } => {
            let ninja = generate_ninja(cli, reporter.as_ref(), None)?;
            if process::is_stdout_path(file.as_path()) {
                process::write_ninja_stdout(&ninja)?;
            } else {
                let output_path = resolve_output_path(cli, file.as_path());
                process::write_ninja_file(output_path.as_ref(), &ninja)?;
            }
            reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_MANIFEST));
            Ok(())
        }
        Commands::Clean => handle_clean(cli, reporter.as_ref()),
        Commands::Graph => handle_graph(cli, reporter.as_ref()),
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
/// use netsuke::status::SilentReporter;
/// let cli = Cli::default();
/// let args = BuildArgs { emit: None, targets: vec![] };
/// handle_build(&cli, &args, &SilentReporter).unwrap();
/// ```
fn handle_build(cli: &Cli, args: &BuildArgs, reporter: &dyn StatusReporter) -> Result<()> {
    let ninja = generate_ninja(
        cli,
        reporter,
        Some(LocalizationKey::new(keys::STATUS_TOOL_BUILD)),
    )?;
    let targets = BuildTargets::new(&args.targets);

    // Normalize the build file path and keep the temporary file alive for the
    // duration of the Ninja invocation. Borrow the emitted path when provided
    // to avoid unnecessary allocation.
    let build_path: Cow<Path>;
    let _tmp_file_guard: Option<NamedTempFile>;
    if let Some(path) = &args.emit {
        let emit_path = resolve_output_path(cli, path.as_path());
        process::write_ninja_file(emit_path.as_ref(), &ninja)?;
        build_path = emit_path;
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
    reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_BUILD));
    Ok(())
}

/// Execute a Ninja tool (e.g., `ninja -t clean`) using a temporary build file.
///
/// Generates the Ninja manifest to a temporary file, then invokes Ninja with
/// `-t <tool>` while preserving the CLI settings (working directory and job
/// count).
///
/// # Errors
///
/// Returns an error if manifest generation or Ninja execution fails.
fn handle_ninja_tool(
    cli: &Cli,
    tool: &str,
    tool_key: LocalizationKey,
    reporter: &dyn StatusReporter,
) -> Result<()> {
    info!(
        target: "netsuke::subcommand",
        subcommand = tool,
        "Preparing Ninja tool invocation"
    );
    let ninja = generate_ninja(cli, reporter, Some(tool_key))?;

    let tmp = process::create_temp_ninja_file(&ninja)?;
    let build_path = tmp.path();

    let program = process::resolve_ninja_program();
    run_ninja_tool(program.as_path(), cli, build_path, tool).with_context(|| {
        format!(
            "running {} -t {} with build file {}",
            program.display(),
            tool,
            build_path.display()
        )
    })?;
    reporter.report_complete(tool_key);
    Ok(())
}

/// Remove build artefacts by invoking `ninja -t clean`.
fn handle_clean(cli: &Cli, reporter: &dyn StatusReporter) -> Result<()> {
    handle_ninja_tool(
        cli,
        "clean",
        LocalizationKey::new(keys::STATUS_TOOL_CLEAN),
        reporter,
    )
}

/// Display build dependency graph by invoking `ninja -t graph`.
fn handle_graph(cli: &Cli, reporter: &dyn StatusReporter) -> Result<()> {
    handle_ninja_tool(
        cli,
        "graph",
        LocalizationKey::new(keys::STATUS_TOOL_GRAPH),
        reporter,
    )
}

/// Generate the Ninja manifest string from the Netsuke manifest referenced by `cli`.
///
/// Reports manifest and graph/synthesis pipeline stages via the provided
/// [`StatusReporter`].
///
/// # Errors
///
/// Returns an error if the manifest cannot be loaded or translated.
///
/// # Examples
/// ```ignore
/// use netsuke::cli::Cli;
/// use netsuke::runner::generate_ninja;
/// use netsuke::status::SilentReporter;
/// let cli = Cli::default();
/// let ninja = generate_ninja(&cli, &SilentReporter).expect("generate");
/// assert!(ninja.as_str().contains("rule"));
/// ```
fn generate_ninja(
    cli: &Cli,
    reporter: &dyn StatusReporter,
    tool_key: Option<LocalizationKey>,
) -> Result<NinjaContent> {
    let manifest_path = resolve_manifest_path(cli)?;
    ensure_manifest_exists_or_error(cli, reporter, &manifest_path)?;

    let policy = cli
        .network_policy()
        .context(localization::message(keys::RUNNER_CONTEXT_NETWORK_POLICY))?;
    let manifest = load_manifest_with_stage_reporting(&manifest_path, policy, reporter)?;
    if tracing::enabled!(tracing::Level::DEBUG) {
        let ast_json = serde_json::to_string_pretty(&manifest).context(localization::message(
            keys::RUNNER_CONTEXT_SERIALISE_MANIFEST,
        ))?;
        debug!("AST:\n{ast_json}");
    }

    report_pipeline_stage(reporter, PipelineStage::IrGenerationValidation, None);
    let graph = BuildGraph::from_manifest(&manifest)
        .context(localization::message(keys::RUNNER_CONTEXT_BUILD_GRAPH))?;

    report_pipeline_stage(
        reporter,
        PipelineStage::NinjaSynthesisAndExecution,
        tool_key,
    );
    let ninja = ninja_gen::generate(&graph)
        .context(localization::message(keys::RUNNER_CONTEXT_GENERATE_NINJA))?;
    Ok(NinjaContent::new(ninja))
}

fn ensure_manifest_exists_or_error(
    cli: &Cli,
    reporter: &dyn StatusReporter,
    manifest_path: &Utf8PathBuf,
) -> Result<()> {
    if manifest_path.as_std_path().exists() {
        return Ok(());
    }

    report_pipeline_stage(reporter, PipelineStage::ManifestIngestion, None);
    // `resolve_manifest_path()` validates that `file_name()` is Some.
    let manifest_name = manifest_path
        .file_name()
        .ok_or_else(|| {
            anyhow!(
                "{}",
                localization::message(keys::RUNNER_MANIFEST_PATH_MISSING_NAME)
                    .with_arg("path", manifest_path.as_str())
            )
        })?
        .to_owned();
    let directory = if cli.directory.is_some() {
        let parent = manifest_path
            .parent()
            .map_or_else(|| manifest_path.as_str(), camino::Utf8Path::as_str);
        localization::message(keys::RUNNER_MANIFEST_DIRECTORY)
            .with_arg("directory", parent)
            .to_string()
    } else {
        localization::message(keys::RUNNER_MANIFEST_CURRENT_DIRECTORY).to_string()
    };
    let message = localization::message(keys::RUNNER_MANIFEST_NOT_FOUND)
        .with_arg("manifest_name", manifest_name.as_str())
        .with_arg("directory", &directory);
    Err(RunnerError::ManifestNotFound {
        manifest_name,
        directory,
        path: manifest_path.to_path_buf().into_std_path_buf(),
        message,
        help: localization::message(keys::RUNNER_MANIFEST_NOT_FOUND_HELP),
    }
    .into())
}

fn load_manifest_with_stage_reporting(
    manifest_path: &Utf8PathBuf,
    policy: crate::stdlib::NetworkPolicy,
    reporter: &dyn StatusReporter,
) -> Result<crate::ast::NetsukeManifest> {
    manifest::from_path_with_policy_and_stage_callback(
        manifest_path.as_std_path(),
        policy,
        |stage| match stage {
            manifest::ManifestLoadStage::ManifestIngestion => {
                report_pipeline_stage(reporter, PipelineStage::ManifestIngestion, None);
            }
            manifest::ManifestLoadStage::InitialYamlParsing => {
                report_pipeline_stage(reporter, PipelineStage::InitialYamlParsing, None);
            }
            manifest::ManifestLoadStage::TemplateExpansion => {
                report_pipeline_stage(reporter, PipelineStage::TemplateExpansion, None);
            }
            manifest::ManifestLoadStage::FinalRendering => {
                report_pipeline_stage(reporter, PipelineStage::FinalRendering, None);
            }
        },
    )
    .with_context(|| {
        localization::message(keys::RUNNER_CONTEXT_LOAD_MANIFEST)
            .with_arg("path", manifest_path.as_str())
    })
}

/// Determine the manifest path respecting the CLI's directory option.
///
/// # Errors
/// Returns an error when the CLI `file` or `directory` paths are not valid UTF-8.
///
/// # Examples
/// ```ignore
/// use crate::cli::Cli;
/// use crate::runner::resolve_manifest_path;
/// let cli = Cli::default();
/// let path = resolve_manifest_path(&cli).expect("valid manifest path");
/// assert!(path.as_str().ends_with("Netsukefile"));
/// ```
fn resolve_manifest_path(cli: &Cli) -> Result<Utf8PathBuf> {
    let file = Utf8PathBuf::from_path_buf(cli.file.clone()).map_err(|path| {
        anyhow!(
            "{}",
            localization::message(keys::RUNNER_MANIFEST_PATH_UTF8)
                .with_arg("path", path.display().to_string())
        )
    })?;
    let resolved = if let Some(dir) = &cli.directory {
        let base = Utf8PathBuf::from_path_buf(dir.clone()).map_err(|path| {
            anyhow!(
                "{}",
                localization::message(keys::RUNNER_MANIFEST_DIR_UTF8)
                    .with_arg("path", path.display().to_string())
            )
        })?;
        base.join(&file)
    } else {
        file
    };
    if resolved.file_name().is_none() {
        return Err(anyhow!(
            "{}",
            localization::message(keys::RUNNER_MANIFEST_PATH_MISSING_NAME)
                .with_arg("path", resolved.as_str())
        ));
    }
    Ok(resolved)
}

/// Resolve an output path relative to the CLI working directory.
///
/// The Netsuke `-C/--directory` option behaves like a working directory change
/// for any filesystem paths supplied on the command line. When `path` is
/// relative and a directory has been configured, the returned path is
/// `directory/path`.
#[must_use]
fn resolve_output_path<'a>(cli: &Cli, path: &'a Path) -> Cow<'a, Path> {
    if path.is_relative() {
        cli.directory
            .as_ref()
            .map_or_else(|| Cow::Borrowed(path), |dir| Cow::Owned(dir.join(path)))
    } else {
        Cow::Borrowed(path)
    }
}

#[cfg(test)]
mod tests;
