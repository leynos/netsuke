//! CLI execution and command dispatch logic.
//!
//! This module keeps `main` minimal by providing a single entry point that
//! handles command execution. It now delegates build requests to the Ninja
//! subprocess, streaming its output back to the user. The executable defaults
//! to `ninja` and may be overridden with `NETSUKE_NINJA` for systems that use a
//! different binary name or require a full path.

mod dispatch;
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
mod path_helpers;
mod process;
#[cfg(doctest)]
pub use process::doc;
pub use process::{
    CommandEnv, NinjaBuildRequest, NinjaToolRequest, StderrMode, run_ninja_tool_with,
    run_ninja_with,
};

use path_helpers::{ensure_manifest_exists_or_error, resolve_manifest_path, resolve_output_path};

/// Runtime dependencies shared by command dispatch handlers.
struct ExecutionContext<'a> {
    reporter: &'a dyn StatusReporter,
    progress_enabled: bool,
    /// Resolved Ninja executable, passed unchanged to [`std::process::Command::new`].
    ///
    /// UTF-8 conversion is confined to `NETSUKE_NINJA` resolution
    /// (`process::resolve_ninja_program`); this field must stay a native
    /// [`Path`] and must not be converted to a `String`, so that non-UTF-8
    /// executable paths on platforms that allow them remain usable.
    ninja_program: &'a Path,
}

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
    let program = process::resolve_ninja_program();
    run_with_ninja_program(cli, prefs, &program)
}

/// Execute parsed commands with an explicitly selected Ninja executable.
///
/// This is the injected process-program boundary used by adapters and tests
/// that must not mutate the process environment to select Ninja.
///
/// # Errors
///
/// Returns an error if manifest generation or the selected Ninja process fails.
pub fn run_with_ninja_program(cli: &Cli, prefs: OutputPrefs, program: &Path) -> Result<()> {
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
    let context = ExecutionContext {
        reporter: reporter.as_ref(),
        progress_enabled,
        ninja_program: program,
    };
    dispatch::execute(cli, command, &context)
}

/// Invoke the Ninja executable with the provided CLI settings.
///
/// Forwards the job count and working directory, specifies the temporary
/// build file, and streams the child's output back to the user. The
/// `stderr_mode` policy is derived from the CLI's JSON diagnostic setting.
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
    run_ninja_with(&NinjaBuildRequest {
        program,
        cli,
        build_file,
        targets,
        env: &CommandEnv::inherit(),
        stderr_mode: StderrMode::from_json_enabled(cli.json),
    })
}

/// Invoke a Ninja tool (e.g., `ninja -t clean`) with the provided CLI settings.
///
/// Forwards the job count and working directory, specifies the build file,
/// and streams the child's output back to the user. The `stderr_mode` policy
/// is derived from the CLI's JSON diagnostic setting.
///
/// # Errors
///
/// Returns an [`io::Error`] if the Ninja process fails to spawn, the standard
/// streams are unavailable, or when Ninja reports a non-zero exit status.
pub fn run_ninja_tool(program: &Path, cli: &Cli, build_file: &Path, tool: &str) -> io::Result<()> {
    run_ninja_tool_with(&NinjaToolRequest {
        program,
        cli,
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
    let ninja = generate_ninja(cli, context.reporter, Some(keys::STATUS_TOOL_BUILD.into()))?;
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
        let mut on_task_progress = on_task_progress_callback(context.reporter);
        process::run_ninja_with_status(
            process::NinjaBuildRequest {
                program: context.ninja_program,
                cli,
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
    context: &ExecutionContext<'_>,
) -> Result<()> {
    info!(
        target: "netsuke::subcommand",
        subcommand = tool.name,
        "Preparing Ninja tool invocation"
    );
    let ninja = generate_ninja(cli, context.reporter, Some(tool.key))?;

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
        let mut on_task_progress = on_task_progress_callback(context.reporter);
        process::run_ninja_tool_with_status(
            process::NinjaToolRequest {
                program: context.ninja_program,
                cli,
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
    context.reporter.report_complete(tool.key);
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
