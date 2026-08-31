//! Dispatch parsed commands and emit their successful JSON result documents.

use super::{
    ExecutionContext, NinjaContent, NinjaToolSpec, generate_ninja_with_shell, graph, handle_build,
    handle_ninja_tool, help, materialize_dyndep_bundle, process, prune_dyndep_bundle,
    resolve_output_path,
};
use crate::cli::{BuildArgs, Cli, Commands, HelpArgs, HelpTopic};
use crate::localization::keys;
use crate::result_json;
use anyhow::{Context, Result};

/// Dispatch a parsed `command` to its subcommand handler.
///
/// # Errors
///
/// Returns an error when the selected subcommand handler fails.
pub(super) fn execute(cli: &Cli, command: Commands, context: &ExecutionContext<'_>) -> Result<()> {
    match command {
        Commands::Build(args) => execute_build(cli, &args, context),
        Commands::Generate { output } => execute_generate(cli, output.as_ref(), context),
        Commands::Clean => execute_clean(cli, context),
        Commands::Graph(args) => graph::handle_graph(cli, &args, context.reporter),
        Commands::Help(args) => execute_help(cli, &args, context.reporter),
    }
}

/// Render help for the requested topic, or the root help when no topic is given.
///
/// # Errors
///
/// Returns an error when the requested help text cannot be rendered or written.
pub(super) fn execute_help(
    cli: &Cli,
    args: &HelpArgs,
    reporter: &dyn crate::status::StatusReporter,
) -> Result<()> {
    match args.topic.as_ref() {
        None => help::render_root_help(),
        Some(HelpTopic::Targets) => help::handle_help_targets(cli, reporter),
        Some(HelpTopic::Build) => help::render_subcommand_help("build"),
        Some(HelpTopic::Clean) => help::render_subcommand_help("clean"),
        Some(HelpTopic::Graph) => help::render_subcommand_help("graph"),
        Some(HelpTopic::Generate) => help::render_subcommand_help("generate"),
    }
}

/// Run the build through Ninja and emit its successful JSON result when
/// `cli.json` is enabled.
///
/// # Errors
///
/// Returns an error when the build fails or the JSON result cannot be written.
fn execute_build(cli: &Cli, args: &BuildArgs, context: &ExecutionContext<'_>) -> Result<()> {
    handle_build(cli, args, context)?;
    write_json_result(cli, "build", None)
}

/// Generate the Ninja bundle and write it to a file, stdout, or JSON.
///
/// The selected output follows the CLI preference: an `--output` path to a
/// file, the rendered Ninja text to stdout, or the JSON envelope.
///
/// # Errors
///
/// Returns an error when generation, sidecar publication, pruning, or output
/// writing fails.
fn execute_generate(
    cli: &Cli,
    output: Option<&std::path::PathBuf>,
    context: &ExecutionContext<'_>,
) -> Result<()> {
    let bundle = generate_ninja_with_shell(cli, context.reporter, None, context.recipe_shell)?;
    let publication = materialize_dyndep_bundle(cli, &bundle)?;
    prune_dyndep_bundle(cli, bundle.dyndep_files(), &publication)?;
    let ninja = NinjaContent::new(bundle.into_parts().0);
    if let Some(file) = output {
        let output_path = resolve_output_path(cli, file.as_path());
        process::write_ninja_file(output_path.as_ref(), &ninja)?;
    } else if cli.json {
        write_json_result(cli, "generate", Some(ninja.as_str()))?;
    } else {
        process::write_ninja_stdout(&ninja)?;
    }
    context
        .reporter
        .report_complete(keys::STATUS_TOOL_GENERATE.into());
    let json_result = if output.is_some() {
        write_json_result(cli, "generate", None)?;
        Ok(())
    } else {
        Ok(())
    };
    drop(publication);
    json_result
}

/// Run Ninja's `clean` tool and emit its successful JSON result.
///
/// # Errors
///
/// Returns an error when the clean tool fails or, when `cli.json` is enabled,
/// the JSON result cannot be written.
fn execute_clean(cli: &Cli, context: &ExecutionContext<'_>) -> Result<()> {
    handle_ninja_tool(
        cli,
        NinjaToolSpec {
            name: "clean",
            key: keys::STATUS_TOOL_CLEAN.into(),
            prune_after_success: true,
        },
        context,
    )?;
    write_json_result(cli, "clean", None)
}

/// Render and write the successful JSON result document when `cli.json` is set.
///
/// # Errors
///
/// Returns an error when JSON serialization or the stdout write fails.
fn write_json_result(cli: &Cli, command: &str, content: Option<&str>) -> Result<()> {
    if !cli.json {
        return Ok(());
    }
    let rendered = result_json::render_result_json(command, content)
        .context("serialize successful command result")?;
    process::write_text_stdout(&rendered)
}
