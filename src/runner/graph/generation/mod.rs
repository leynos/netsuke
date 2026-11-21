//! Compose manifest-to-IR graph generation with runner status and telemetry.
//!
//! This runner-internal boundary combines the selected recipe shell, monotonic
//! clock, pipeline reporting, and observability. It keeps the pure generation
//! queries in [`super::super::generation`] free of runner infrastructure.

mod telemetry;

use super::super::manifest_structure_telemetry::record_manifest_structure;
use super::super::{
    Cli, Context, LocalizationKey, PipelineStage, Result, StatusReporter, dyndep, generation,
    load_manifest_with_stage_reporting, path_helpers, report_pipeline_stage,
};
use crate::ir::BuildGraph;
use crate::localization::{self, keys};
use crate::ninja_gen::GeneratedNinja;
use crate::recipe_shell::InterpreterExecutable;
use crate::runner::process::command_list_targets::CommandListTargetIndex;
use camino::Utf8PathBuf;
use monotony::MonotonicClock;

/// Supply the dependencies that select and measure graph generation.
///
/// Keep this runner-internal composition boundary limited to the build
/// pipeline. The same clock also measures its Ninja invocation.
pub(in crate::runner) struct GraphGenerationContext<'a> {
    /// Select the legacy-recipe interpreter used during graph generation.
    pub(in crate::runner) recipe_shell: crate::recipe_shell::RecipeShell,
    /// Bind generated recipes to the interpreter the runner resolved.
    pub(in crate::runner) interpreter_binding: Option<InterpreterExecutable>,
    /// Measure graph generation and Ninja execution with a runner-provided clock.
    pub(in crate::runner) clock: &'a dyn MonotonicClock,
}

impl GraphGenerationContext<'_> {
    /// Attach the interpreter binding resolved for one executing operation.
    #[must_use]
    pub(in crate::runner) fn with_interpreter_binding(
        &self,
        interpreter_binding: Option<InterpreterExecutable>,
    ) -> Self {
        GraphGenerationContext {
            recipe_shell: self.recipe_shell,
            interpreter_binding,
            clock: self.clock,
        }
    }
}

/// Own a generated Ninja bundle and its invocation-local target lookup.
///
/// The lookup contains only bounded output names and fingerprints; it does not
/// keep the build graph alive after generation. CLI builds retain it through
/// their synchronous Ninja call, while generate and Ninja-tool operations
/// discard it with [`Self::into_bundle`].
pub(in crate::runner) struct GeneratedNinjaWithTargets {
    /// Main Ninja file and any sidecars produced for this invocation.
    bundle: GeneratedNinja,
    /// Bounded output names keyed by the generated command-list fingerprint.
    command_list_targets: CommandListTargetIndex,
    /// Every path the manifest named, in the graph's own spelling.
    ///
    /// The build command consults them before it removes or creates generated
    /// state, so a spelling that only the working directory can resolve cannot
    /// collide with it.
    declared_paths: Vec<Utf8PathBuf>,
}

/// Everything one CLI build needs from a generated bundle.
///
/// The fields travel together because the build command needs all four before
/// it invokes Ninja, and two adjacent path lists are easy to swap when they are
/// returned as a tuple.
pub(in crate::runner) struct BuildParts {
    /// Main Ninja file and any sidecars produced for this invocation.
    pub(in crate::runner) bundle: GeneratedNinja,
    /// Bounded output names keyed by the generated command-list fingerprint.
    pub(in crate::runner) command_list_targets: CommandListTargetIndex,
    /// Every path the manifest named, in the graph's own spelling.
    pub(in crate::runner) declared_paths: Vec<Utf8PathBuf>,
}

impl GeneratedNinjaWithTargets {
    /// Return the generated Ninja bundle and discard its lookup.
    pub(in crate::runner) fn into_bundle(self) -> GeneratedNinja {
        self.bundle
    }

    /// Return the bundle and the prepared paths for one CLI build.
    pub(in crate::runner) fn into_build_parts(self) -> BuildParts {
        BuildParts {
            bundle: self.bundle,
            command_list_targets: self.command_list_targets,
            declared_paths: self.declared_paths,
        }
    }
}

/// Collect every path the manifest declares, in the graph's own spelling.
fn declared_paths(graph: &BuildGraph) -> Vec<Utf8PathBuf> {
    graph
        .edges()
        .flat_map(|edge| {
            edge.explicit_outputs
                .iter()
                .chain(&edge.implicit_outputs)
                .chain(&edge.inputs)
                .chain(&edge.implicit_deps)
                .chain(&edge.order_only_deps)
        })
        .chain(&graph.default_targets)
        .cloned()
        .collect()
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
pub(in crate::runner) fn generate_ninja_with_shell(
    cli: &Cli,
    reporter: &dyn StatusReporter,
    tool_key: Option<LocalizationKey>,
    graph_generation: &GraphGenerationContext<'_>,
) -> Result<GeneratedNinjaWithTargets> {
    let manifest_path = path_helpers::resolve_manifest_path(cli)?;
    path_helpers::ensure_manifest_exists_or_error(cli, reporter, &manifest_path)?;

    let inputs = generation::ManifestLoadInputs::from_cli(cli, graph_generation.recipe_shell)?;
    let manifest = load_manifest_with_stage_reporting(&manifest_path, &inputs, reporter)?;
    record_manifest_structure(&manifest);

    report_pipeline_stage(reporter, PipelineStage::IrGenerationValidation, None);
    let graph = self::telemetry::instrument_graph_generation(
        graph_generation.clock,
        graph_generation.recipe_shell,
        || generation::build_graph_for_shell(&manifest, graph_generation.recipe_shell),
    )
    .context(localization::message(keys::RUNNER_CONTEXT_BUILD_GRAPH))?;
    let command_list_targets = CommandListTargetIndex::from_graph(&graph);

    report_pipeline_stage(
        reporter,
        PipelineStage::NinjaSynthesisAndExecution,
        tool_key,
    );
    let bundle = dyndep::generation_telemetry::instrument_bundle_generation(&graph, || {
        generation::ninja_text_for_shell_with_interpreter(
            &graph,
            graph_generation.recipe_shell,
            graph_generation.interpreter_binding.as_ref(),
        )
    })
    .context(localization::message(keys::RUNNER_CONTEXT_GENERATE_NINJA))?;
    Ok(GeneratedNinjaWithTargets {
        bundle,
        command_list_targets,
        declared_paths: declared_paths(&graph),
    })
}
