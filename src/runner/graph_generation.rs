//! Compose manifest-to-IR graph generation with runner status and telemetry.
//!
//! This runner-internal boundary combines the selected recipe shell, monotonic
//! clock, pipeline reporting, and observability. It keeps the pure generation
//! queries in [`super::generation`] free of runner infrastructure.

use super::{
    Cli, Context, LocalizationKey, PipelineStage, Result, StatusReporter,
    dyndep_generation_telemetry, generation, graph_generation_telemetry,
    load_manifest_with_stage_reporting, path_helpers, recipe_shell, report_pipeline_stage,
};
use crate::localization::{self, keys};
use crate::ninja_gen;
use monotony::MonotonicClock;
use tracing::debug;

/// Supply the dependencies that select and measure graph generation.
///
/// Keep this runner-internal composition boundary limited to graph generation
/// so unrelated command dispatch does not acquire a clock dependency.
pub(super) struct GraphGenerationContext<'a> {
    /// Select the legacy-recipe interpreter used during graph generation.
    pub(super) recipe_shell: crate::recipe_shell::RecipeShell,
    /// Measure graph generation with a runner-provided monotonic clock.
    pub(super) clock: &'a dyn MonotonicClock,
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
/// Generate Ninja output using one selected legacy-recipe interpreter.
pub(super) fn generate_ninja_with_shell(
    cli: &Cli,
    reporter: &dyn StatusReporter,
    tool_key: Option<LocalizationKey>,
    graph_generation: &GraphGenerationContext<'_>,
) -> Result<ninja_gen::GeneratedNinja> {
    recipe_shell::validate_recipe_shell(graph_generation.recipe_shell)?;
    let manifest_path = path_helpers::resolve_manifest_path(cli)?;
    path_helpers::ensure_manifest_exists_or_error(cli, reporter, &manifest_path)?;

    let policy = cli
        .network_policy()
        .context(localization::message(keys::RUNNER_CONTEXT_NETWORK_POLICY))?;
    let budget_limits = cli.manifest_budget_limits()?;
    let manifest =
        load_manifest_with_stage_reporting(&manifest_path, policy, budget_limits, reporter)?;
    if tracing::enabled!(tracing::Level::DEBUG) {
        let ast_json = serde_json::to_string_pretty(&manifest).context(localization::message(
            keys::RUNNER_CONTEXT_SERIALISE_MANIFEST,
        ))?;
        debug!("AST:\n{ast_json}");
    }

    report_pipeline_stage(reporter, PipelineStage::IrGenerationValidation, None);
    let graph = graph_generation_telemetry::instrument_graph_generation(
        graph_generation.clock,
        graph_generation.recipe_shell,
        || generation::build_graph_for_shell(&manifest, graph_generation.recipe_shell),
    )
    .context(localization::message(keys::RUNNER_CONTEXT_BUILD_GRAPH))?;

    report_pipeline_stage(
        reporter,
        PipelineStage::NinjaSynthesisAndExecution,
        tool_key,
    );
    dyndep_generation_telemetry::instrument_bundle_generation(&graph, || {
        generation::ninja_text_for_shell(&graph, graph_generation.recipe_shell)
    })
    .context(localization::message(keys::RUNNER_CONTEXT_GENERATE_NINJA))
}
