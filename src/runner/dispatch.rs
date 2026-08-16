//! Dispatch parsed commands and emit their successful JSON result documents.

use super::{
    ExecutionContext, NinjaToolSpec, generate_ninja, graph, handle_build, handle_ninja_tool, help,
    process, resolve_output_path,
};
use crate::cli::{BuildArgs, Cli, Commands, HelpArgs, HelpTopic};
use crate::localization::keys;
use crate::result_json;
use anyhow::{Context, Result};

pub(super) fn execute(cli: &Cli, command: Commands, context: &ExecutionContext<'_>) -> Result<()> {
    match command {
        Commands::Build(args) => execute_build(cli, &args, context),
        Commands::Generate { output } => execute_generate(cli, output.as_ref(), context),
        Commands::Clean => execute_clean(cli, context),
        Commands::Graph(args) => graph::handle_graph(cli, &args, context.reporter),
        Commands::Help(args) => execute_help(cli, &args, context.reporter),
    }
}

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

fn execute_build(cli: &Cli, args: &BuildArgs, context: &ExecutionContext<'_>) -> Result<()> {
    handle_build(cli, args, context)?;
    write_json_result(cli, "build", None)
}

fn execute_generate(
    cli: &Cli,
    output: Option<&std::path::PathBuf>,
    context: &ExecutionContext<'_>,
) -> Result<()> {
    let ninja = generate_ninja(cli, context.reporter, None)?;
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
    if output.is_some() {
        write_json_result(cli, "generate", None)
    } else {
        Ok(())
    }
}

fn execute_clean(cli: &Cli, context: &ExecutionContext<'_>) -> Result<()> {
    handle_ninja_tool(
        cli,
        NinjaToolSpec {
            name: "clean",
            key: keys::STATUS_TOOL_CLEAN.into(),
        },
        context,
    )?;
    write_json_result(cli, "clean", None)
}

fn write_json_result(cli: &Cli, command: &str, content: Option<&str>) -> Result<()> {
    if !cli.json {
        return Ok(());
    }
    let rendered = result_json::render_result_json(command, content)
        .context("serialize successful command result")?;
    process::write_text_stdout(&rendered)
}
