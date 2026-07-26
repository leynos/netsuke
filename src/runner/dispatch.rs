//! Dispatch parsed commands and emit their successful JSON result documents.

use super::{
    NinjaToolSpec, generate_ninja, graph, handle_build, handle_ninja_tool, process,
    resolve_output_path,
};
use crate::cli::{BuildArgs, Cli, Commands};
use crate::localization::keys;
use crate::result_json;
use crate::status::StatusReporter;
use anyhow::{Context, Result};

pub(super) fn execute(
    cli: &Cli,
    command: Commands,
    reporter: &dyn StatusReporter,
    progress_enabled: bool,
) -> Result<()> {
    match command {
        Commands::Build(args) => execute_build(cli, &args, reporter, progress_enabled),
        Commands::Generate { output } => execute_generate(cli, output.as_ref(), reporter),
        Commands::Clean => execute_clean(cli, reporter, progress_enabled),
        Commands::Graph(args) => graph::handle_graph(cli, &args, reporter),
    }
}

fn execute_build(
    cli: &Cli,
    args: &BuildArgs,
    reporter: &dyn StatusReporter,
    progress_enabled: bool,
) -> Result<()> {
    handle_build(cli, args, reporter, progress_enabled)?;
    write_json_result(cli, "build", None)
}

fn execute_generate(
    cli: &Cli,
    output: Option<&std::path::PathBuf>,
    reporter: &dyn StatusReporter,
) -> Result<()> {
    let ninja = generate_ninja(cli, reporter, None)?;
    if let Some(file) = output {
        let output_path = resolve_output_path(cli, file.as_path());
        process::write_ninja_file(output_path.as_ref(), &ninja)?;
    } else if cli.json {
        write_json_result(cli, "generate", Some(ninja.as_str()))?;
    } else {
        process::write_ninja_stdout(&ninja)?;
    }
    reporter.report_complete(keys::STATUS_TOOL_GENERATE.into());
    if output.is_some() {
        write_json_result(cli, "generate", None)
    } else {
        Ok(())
    }
}

fn execute_clean(cli: &Cli, reporter: &dyn StatusReporter, progress_enabled: bool) -> Result<()> {
    handle_ninja_tool(
        cli,
        NinjaToolSpec {
            name: "clean",
            key: keys::STATUS_TOOL_CLEAN.into(),
        },
        reporter,
        progress_enabled,
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
