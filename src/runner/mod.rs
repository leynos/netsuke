//! CLI execution and command dispatch.
//!
//! Provides execution orchestration; build work streams through Ninja (default
//! `ninja`, overridable with `NETSUKE_NINJA`).

mod dispatch;
mod dyndep_generation_telemetry;
mod dyndep_publication;
mod error;
mod reporter;

pub use error::RunnerError;

use crate::cli::{BuildArgs, Cli, Commands};
use crate::localization::{self, keys};
use crate::output_mode;
use crate::output_prefs::OutputPrefs;
use crate::status::{LocalizationKey, PipelineStage, StatusReporter, report_pipeline_stage};
use crate::{ir::BuildGraph, manifest, ninja_gen};
use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use std::borrow::Cow;
use std::io::{self, IsTerminal};
use std::path::Path;
use tracing::{debug, info};

/// Default Ninja executable to invoke.
pub const NINJA_PROGRAM: &str = "ninja";
/// Environment variable override for the Ninja executable.
///
/// # Examples
///
/// ```rust,no_run
/// use netsuke::runner::NINJA_ENV;
///
/// if let Ok(ninja) = std::env::var(NINJA_ENV) {
///     println!("using ninja override: {ninja}");
/// }
/// ```
pub const NINJA_ENV: &str = "NETSUKE_NINJA";

mod graph;
mod help;
mod ninja_content;
mod path_helpers;
mod process;
pub use ninja_content::NinjaContent;
#[cfg(doctest)]
pub use process::doc;
pub use process::{
    CommandEnv, MAX_RETAINED_DYNDEP_FILES, NinjaBuildRequest, NinjaProcessOptions, NinjaToolRequest,
    StderrMode, run_ninja_tool_with, run_ninja_with,
};

use dyndep_publication::{materialize_dyndep_bundle, prune_dyndep_bundle};
use path_helpers::{ensure_manifest_exists_or_error, resolve_manifest_path, resolve_output_path};

/// Runtime dependencies shared by command dispatch handlers.
struct ExecutionContext<'a> {
    reporter: &'a dyn StatusReporter,
    progress_enabled: bool,
    /// Resolved Ninja executable passed unchanged to [`std::process::Command::new`].
    ///
    /// Keep a native [`Path`]: only `NETSUKE_NINJA` resolution performs UTF-8
    /// conversion, preserving valid non-UTF-8 executable paths.
    ninja_program: &'a Path,
}

/// Target list passed through to Ninja; an empty slice uses IR defaults.
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

/// Execute the parsed [`Cli`] commands with the given output preferences.
///
/// # Errors
///
/// Returns an error if manifest generation or the Ninja process fails.
pub fn run(cli: &Cli, prefs: OutputPrefs) -> Result<()> {
    run_with_ninja_program_resolver(cli, prefs, None, process::resolve_ninja_program)
}

/// Execute parsed commands with a Ninja executable selected by the caller.
///
/// # Errors
///
/// Returns an error if manifest generation or the selected Ninja process fails.
pub fn run_with_ninja_program(cli: &Cli, prefs: OutputPrefs, program: &Path) -> Result<()> {
    run_with_ninja_program_resolver(cli, prefs, Some(program), || program.to_path_buf())
}

/// Translate CLI state into the narrow options consumed by the process layer.
fn ninja_process_options(cli: &Cli) -> NinjaProcessOptions {
    NinjaProcessOptions {
        working_dir: cli.directory.clone(),
        jobs: cli.jobs,
    }
}

/// Dispatch a command after resolving Ninja only for commands that require it.
fn run_with_ninja_program_resolver(
    cli: &Cli,
    prefs: OutputPrefs,
    configured_program: Option<&Path>,
    resolve_program: impl FnOnce() -> std::path::PathBuf,
) -> Result<()> {
    let mode = output_mode::resolve(cli.accessibility_override(), Some(cli.color));
    let progress_enabled = cli.progress_enabled() && !cli.json;
    let stdout_is_tty = std::io::stdout().is_terminal();
    let reporter = reporter::make_reporter(reporter::ReporterOptions {
        mode,
        progress_enabled,
        verbose: cli.verbose && !cli.json,
        prefs,
        stdout_is_tty,
    });

    let command = cli.command.clone().unwrap_or(Commands::Build(BuildArgs {
        targets: Vec::new(),
    }));
    if let Commands::Help(args) = &command {
        return dispatch::execute_help(cli, args, reporter.as_ref());
    }
    let ninja_program =
        configured_program.map_or_else(|| Cow::Owned(resolve_program()), Cow::Borrowed);
    let context = ExecutionContext {
        reporter: reporter.as_ref(),
        progress_enabled,
        ninja_program: ninja_program.as_ref(),
    };
    dispatch::execute(cli, command, &context)
}

/// Invoke the Ninja executable with the provided CLI settings.
///
/// Forwards the job count and working directory and specifies the temporary
/// build file. Child output follows the `stderr_mode` policy derived from the
/// CLI's JSON diagnostic setting: `StderrMode::Suppress` drains both child
/// streams to a sink so JSON output stays machine-readable, while
/// `StderrMode::Forward` relays them to the user.
///
/// # Errors
///
/// Returns an [`io::Error`] if the Ninja process fails to spawn, the standard
/// streams are unavailable, or when Ninja reports a non-zero exit status.
pub fn run_ninja(
    program: &Path,
    cli: &Cli,
    build_file: &Path,
    targets: &BuildTargets<'_>,
) -> io::Result<()> {
    let options = ninja_process_options(cli);
    run_ninja_with(&NinjaBuildRequest {
        program,
        options: &options,
        build_file,
        targets,
        env: &CommandEnv::inherit(),
        stderr_mode: StderrMode::from_json_enabled(cli.json),
    })
}

/// Invoke a Ninja tool (e.g., `ninja -t clean`) with the provided CLI settings.
///
/// Forwards the job count and working directory and specifies the build file.
/// Child output follows the `stderr_mode` policy derived from the CLI's JSON
/// diagnostic setting: `StderrMode::Suppress` drains both child streams to a
/// sink, while `StderrMode::Forward` relays them to the user.
///
/// # Errors
///
/// Returns an [`io::Error`] if the Ninja process fails to spawn, the standard
/// streams are unavailable, or when Ninja reports a non-zero exit status.
pub fn run_ninja_tool(program: &Path, cli: &Cli, build_file: &Path, tool: &str) -> io::Result<()> {
    let options = ninja_process_options(cli);
    run_ninja_tool_with(&NinjaToolRequest {
        program,
        options: &options,
        build_file,
        tool,
        env: &CommandEnv::inherit(),
        stderr_mode: StderrMode::from_json_enabled(cli.json),
    })
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
fn handle_build(cli: &Cli, args: &BuildArgs, context: &ExecutionContext<'_>) -> Result<()> {
    let bundle = generate_ninja(cli, context.reporter, Some(keys::STATUS_TOOL_BUILD.into()))?;
    let publication = materialize_dyndep_bundle(cli, &bundle)?;
    prune_dyndep_bundle(cli, bundle.dyndep_files(), &publication)?;
    let ninja = NinjaContent::new(bundle.into_parts().0);
    let targets = if args.targets.is_empty() {
        BuildTargets::new(&cli.default_targets)
    } else {
        BuildTargets::new(&args.targets)
    };

    let build_file = process::create_temp_ninja_file(&ninja)?;
    let build_path = build_file.path();

    let ctx = || {
        format!(
            "running {} with build file {}",
            context.ninja_program.display(),
            build_path.display()
        )
    };
    if context.progress_enabled {
        let options = ninja_process_options(cli);
        let mut on_task_progress = on_task_progress_callback(context.reporter);
        process::run_ninja_with_status(
            process::NinjaBuildRequest {
                program: context.ninja_program,
                options: &options,
                build_file: build_path,
                targets: &targets,
                env: &CommandEnv::inherit(),
                stderr_mode: StderrMode::from_json_enabled(cli.json),
            },
            &mut on_task_progress,
        )
        .with_context(ctx)?;
    } else {
        run_ninja(context.ninja_program, cli, build_path, &targets).with_context(ctx)?;
    }
    context
        .reporter
        .report_complete(keys::STATUS_TOOL_BUILD.into());
    drop(publication);
    Ok(())
}

/// Specification for a Ninja tool invocation: name and localization key.
#[derive(Clone, Copy)]
struct NinjaToolSpec<'a> {
    name: &'a str,
    key: LocalizationKey,
    prune_after_success: bool,
}

/// Execute a Ninja tool using a temporary build file and CLI settings.
///
/// # Errors
///
/// Returns an error if manifest generation or Ninja execution fails.
fn handle_ninja_tool(
    cli: &Cli,
    tool: NinjaToolSpec<'_>,
    context: &ExecutionContext<'_>,
) -> Result<()> {
    info!(
        target: "netsuke::subcommand",
        subcommand = tool.name,
        "Preparing Ninja tool invocation"
    );
    let bundle = generate_ninja(cli, context.reporter, Some(tool.key))?;
    let publication = materialize_dyndep_bundle(cli, &bundle)?;
    let (ninja_file, dyndep_files) = bundle.into_parts();
    let ninja = NinjaContent::new(ninja_file);

    let tmp = process::create_temp_ninja_file(&ninja)?;
    let build_path = tmp.path();

    let ctx = || {
        format!(
            "running {} -t {} with build file {}",
            context.ninja_program.display(),
            tool.name,
            build_path.display()
        )
    };
    if context.progress_enabled {
        let options = ninja_process_options(cli);
        let mut on_task_progress = on_task_progress_callback(context.reporter);
        process::run_ninja_tool_with_status(
            process::NinjaToolRequest {
                program: context.ninja_program,
                options: &options,
                build_file: build_path,
                tool: tool.name,
                env: &CommandEnv::inherit(),
                stderr_mode: StderrMode::from_json_enabled(cli.json),
            },
            &mut on_task_progress,
        )
        .with_context(ctx)?;
    } else {
        run_ninja_tool(context.ninja_program, cli, build_path, tool.name).with_context(ctx)?;
    }
    if tool.prune_after_success {
        prune_dyndep_bundle(cli, &dyndep_files, &publication)?;
    }
    context.reporter.report_complete(tool.key);
    drop(publication);
    Ok(())
}

/// Generate a Ninja bundle from the manifest referenced by `cli`.
///
/// # Errors
///
/// Returns an error if the manifest cannot be loaded or translated.
///
/// # Examples
/// ```ignore
/// use netsuke::cli::Cli;
/// use netsuke::ninja_gen::GeneratedNinja;
/// # let _: Option<GeneratedNinja> = None;
/// ```
fn generate_ninja(
    cli: &Cli,
    reporter: &dyn StatusReporter,
    tool_key: Option<LocalizationKey>,
) -> Result<ninja_gen::GeneratedNinja> {
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
    dyndep_generation_telemetry::instrument_bundle_generation(&graph, || {
        ninja_gen::generate_bundle(&graph)
    })
    .context(localization::message(keys::RUNNER_CONTEXT_GENERATE_NINJA))
}

pub(super) fn load_manifest_with_stage_reporting(
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
