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
use crate::output_prefs::OutputPrefs;
use crate::status::{
    AccessibleReporter, IndicatifReporter, LocalizationKey, PipelineStage, SilentReporter,
    StatusReporter, report_pipeline_stage,
};
use crate::{ir::BuildGraph, manifest, ninja_gen};
use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use std::borrow::Cow;
use std::io::IsTerminal;
use std::path::Path;
use tempfile::NamedTempFile;
use tracing::{debug, info};

/// Default Ninja executable to invoke.
pub const NINJA_PROGRAM: &str = "ninja";
/// Environment variable override for the Ninja executable.
pub use ninja_env::NINJA_ENV;

mod path_helpers;
mod process;
#[cfg(doctest)]
pub use process::doc;
pub use process::{run_ninja, run_ninja_tool};

use path_helpers::{ensure_manifest_exists_or_error, resolve_manifest_path, resolve_output_path};

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

/// Build the appropriate [`StatusReporter`] for the resolved output mode,
/// progress preference, and output preferences.
fn make_reporter(
    mode: OutputMode,
    progress_enabled: bool,
    prefs: OutputPrefs,
    stdout_is_tty: bool,
) -> Box<dyn StatusReporter> {
    if !progress_enabled {
        return Box::new(SilentReporter);
    }
    let force_text_task_updates = should_force_text_task_updates(mode, stdout_is_tty);
    match mode {
        OutputMode::Accessible => Box::new(AccessibleReporter::new(prefs)),
        OutputMode::Standard => Box::new(IndicatifReporter::with_force_text_task_updates(
            force_text_task_updates,
        )),
    }
}

const fn should_force_text_task_updates(mode: OutputMode, stdout_is_tty: bool) -> bool {
    mode.is_accessible() || !stdout_is_tty
}

/// Execute the parsed [`Cli`] commands with the given output preferences.
///
/// # Errors
///
/// Returns an error if manifest generation or the Ninja process fails.
pub fn run(cli: &Cli, prefs: OutputPrefs) -> Result<()> {
    let mode = output_mode::resolve(cli.accessible);
    let progress_enabled = cli.progress.unwrap_or(true);
    let stdout_is_tty = std::io::stdout().is_terminal();
    let reporter = make_reporter(mode, progress_enabled, prefs, stdout_is_tty);

    let command = cli.command.clone().unwrap_or(Commands::Build(BuildArgs {
        emit: None,
        targets: Vec::new(),
    }));
    match command {
        Commands::Build(args) => handle_build(cli, &args, reporter.as_ref(), progress_enabled),
        Commands::Manifest { file } => {
            let ninja = generate_ninja(cli, reporter.as_ref(), None)?;
            if process::is_stdout_path(file.as_path()) {
                process::write_ninja_stdout(&ninja)?;
            } else {
                let output_path = resolve_output_path(cli, file.as_path());
                process::write_ninja_file(output_path.as_ref(), &ninja)?;
            }
            reporter.report_complete(keys::STATUS_TOOL_MANIFEST.into());
            Ok(())
        }
        Commands::Clean => handle_ninja_tool(
            cli,
            NinjaToolSpec {
                name: "clean",
                key: keys::STATUS_TOOL_CLEAN.into(),
            },
            reporter.as_ref(),
            progress_enabled,
        ),
        Commands::Graph => handle_ninja_tool(
            cli,
            NinjaToolSpec {
                name: "graph",
                key: keys::STATUS_TOOL_GRAPH.into(),
            },
            reporter.as_ref(),
            progress_enabled,
        ),
    }
}

fn on_task_progress_callback(reporter: &dyn StatusReporter) -> impl FnMut(u32, u32, &str) + '_ {
    move |current: u32, total: u32, description: &str| {
        reporter.report_task_progress(current, total, description);
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
/// let progress_enabled = true;
/// handle_build(&cli, &args, &SilentReporter, progress_enabled).unwrap();
/// ```
fn handle_build(
    cli: &Cli,
    args: &BuildArgs,
    reporter: &dyn StatusReporter,
    progress_enabled: bool,
) -> Result<()> {
    let ninja = generate_ninja(cli, reporter, Some(keys::STATUS_TOOL_BUILD.into()))?;
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
    let ctx = || {
        format!(
            "running {} with build file {}",
            program.display(),
            build_path.display()
        )
    };
    if progress_enabled {
        let mut on_task_progress = on_task_progress_callback(reporter);
        process::run_ninja_with_status(
            process::NinjaBuildRequest {
                program: program.as_path(),
                cli,
                build_file: build_path.as_ref(),
                targets: &targets,
            },
            &mut on_task_progress,
        )
        .with_context(ctx)?;
    } else {
        run_ninja(program.as_path(), cli, build_path.as_ref(), &targets).with_context(ctx)?;
    }
    reporter.report_complete(keys::STATUS_TOOL_BUILD.into());
    Ok(())
}

/// Specification for a Ninja tool invocation: name and localization key.
#[derive(Clone, Copy)]
struct NinjaToolSpec<'a> {
    name: &'a str,
    key: LocalizationKey,
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
    tool: NinjaToolSpec<'_>,
    reporter: &dyn StatusReporter,
    progress_enabled: bool,
) -> Result<()> {
    info!(
        target: "netsuke::subcommand",
        subcommand = tool.name,
        "Preparing Ninja tool invocation"
    );
    let ninja = generate_ninja(cli, reporter, Some(tool.key))?;

    let tmp = process::create_temp_ninja_file(&ninja)?;
    let build_path = tmp.path();

    let program = process::resolve_ninja_program();
    let ctx = || {
        format!(
            "running {} -t {} with build file {}",
            program.display(),
            tool.name,
            build_path.display()
        )
    };
    if progress_enabled {
        let mut on_task_progress = on_task_progress_callback(reporter);
        process::run_ninja_tool_with_status(
            process::NinjaToolRequest {
                program: program.as_path(),
                cli,
                build_file: build_path,
                tool: tool.name,
            },
            &mut on_task_progress,
        )
        .with_context(ctx)?;
    } else {
        run_ninja_tool(program.as_path(), cli, build_path, tool.name).with_context(ctx)?;
    }
    reporter.report_complete(tool.key);
    Ok(())
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
/// let ninja = generate_ninja(&cli, &SilentReporter, None).expect("generate");
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

fn load_manifest_with_stage_reporting(
    manifest_path: &Utf8PathBuf,
    policy: crate::stdlib::NetworkPolicy,
    reporter: &dyn StatusReporter,
) -> Result<crate::ast::NetsukeManifest> {
    let mut on_stage = |stage: manifest::ManifestLoadStage| match stage {
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
    };
    manifest::from_path_with_policy(manifest_path.as_std_path(), policy, Some(&mut on_stage))
        .with_context(|| {
            localization::message(keys::RUNNER_CONTEXT_LOAD_MANIFEST)
                .with_arg("path", manifest_path.as_str())
        })
}

#[cfg(test)]
mod tests;
