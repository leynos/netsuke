//! Dispatch and artefact-writing for the `graph` subcommand.
//!
//! The runner builds the in-process [`BuildGraph`], projects it through
//! [`GraphView`], and hands the deterministic projection to one of two
//! adapters: [`DotRenderer`] by default, [`HtmlRenderer`] when `--html` is
//! set. The rendered artefact is then written via the shared
//! [`super::process`] sinks, honouring the `-` stdout sentinel and
//! `-C/--directory` resolution for relative `--output` paths.

use anyhow::{Context, Result};
use std::path::Path;
use tracing::info;

use crate::cli::{Cli, GraphArgs};
use crate::graph_view::GraphView;
use crate::graph_view::render::GraphRenderer;
use crate::graph_view::render_dot::DotRenderer;
use crate::graph_view::render_html::HtmlRenderer;
use crate::ir::BuildGraph;
use crate::localization::{self, keys};
use crate::status::{LocalizationKey, PipelineStage, StatusReporter, report_pipeline_stage};

use super::path_helpers::{
    ensure_manifest_exists_or_error, resolve_manifest_path, resolve_output_path,
};
use super::{load_manifest_with_stage_reporting, process};

pub(super) fn handle_graph(
    cli: &Cli,
    args: &GraphArgs,
    reporter: &dyn StatusReporter,
) -> Result<()> {
    info!(
        target: "netsuke::subcommand",
        subcommand = "graph",
        html = args.html,
        "Rendering build graph in-process"
    );
    let manifest_path = resolve_manifest_path(cli)?;
    ensure_manifest_exists_or_error(cli, reporter, &manifest_path)?;
    let policy = cli
        .network_policy()
        .context(localization::message(keys::RUNNER_CONTEXT_NETWORK_POLICY))?;
    let manifest = load_manifest_with_stage_reporting(&manifest_path, policy, reporter)?;
    report_pipeline_stage(reporter, PipelineStage::IrGenerationValidation, None);
    let graph = BuildGraph::from_manifest(&manifest)
        .context(localization::message(keys::RUNNER_CONTEXT_BUILD_GRAPH))?;
    let view = GraphView::from_build_graph(&graph);

    let status_key: LocalizationKey = if args.html {
        keys::STATUS_TOOL_GRAPH_HTML.into()
    } else {
        keys::STATUS_TOOL_GRAPH.into()
    };
    report_pipeline_stage(reporter, PipelineStage::GraphRendering, Some(status_key));

    let mut buffer: Vec<u8> = Vec::new();
    if args.html {
        HtmlRenderer::new(cli.locale.as_deref())
            .render(&view, &mut buffer)
            .context(localization::message(keys::RUNNER_CONTEXT_RENDER_GRAPH))?;
    } else {
        DotRenderer::new()
            .render(&view, &mut buffer)
            .context(localization::message(keys::RUNNER_CONTEXT_RENDER_GRAPH))?;
    }
    let rendered = String::from_utf8(buffer)
        .context(localization::message(keys::RUNNER_CONTEXT_RENDER_GRAPH))?;

    write_graph_artefact(cli, args.output.as_deref(), &rendered)?;
    reporter.report_complete(status_key);
    Ok(())
}

fn write_graph_artefact(cli: &Cli, output: Option<&Path>, content: &str) -> Result<()> {
    match output {
        None => process::write_text_stdout(content),
        Some(path) if process::is_stdout_path(path) => process::write_text_stdout(content),
        Some(path) => {
            let resolved = resolve_output_path(cli, path);
            process::write_text_file(resolved.as_ref(), content)
        }
    }
}
