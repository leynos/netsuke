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
use crate::localization::{self, keys};
use crate::ninja_gen::GeneratedNinja;
use crate::recipe_shell::BashExecutable;
use crate::runner::process::command_list_targets::CommandListTargetIndex;
use monotony::MonotonicClock;

/// Supply the dependencies that select and measure graph generation.
///
/// Keep this runner-internal composition boundary limited to graph generation
/// so unrelated command dispatch does not acquire a clock dependency.
pub(in crate::runner) struct GraphGenerationContext<'a> {
    /// Select the legacy-recipe interpreter used during graph generation.
    pub(in crate::runner) recipe_shell: crate::recipe_shell::RecipeShell,
    /// Bind builds to the Bash executable that passed runner preflight.
    pub(in crate::runner) bash_executable: Option<BashExecutable>,
    /// Measure graph generation with a runner-provided monotonic clock.
    pub(in crate::runner) clock: &'a dyn MonotonicClock,
}

impl GraphGenerationContext<'_> {
    /// Attach the Bash executable resolved for one build or Ninja-tool operation.
    #[must_use]
    pub(in crate::runner) fn with_bash_executable(
        &self,
        bash_executable: Option<BashExecutable>,
    ) -> Self {
        GraphGenerationContext {
            recipe_shell: self.recipe_shell,
            bash_executable,
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
}

impl GeneratedNinjaWithTargets {
    /// Return the generated Ninja bundle and discard its lookup.
    pub(in crate::runner) fn into_bundle(self) -> GeneratedNinja {
        self.bundle
    }

    /// Return the bundle and lookup for one synchronous CLI build.
    pub(in crate::runner) fn into_build_parts(self) -> (GeneratedNinja, CommandListTargetIndex) {
        (self.bundle, self.command_list_targets)
    }
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
        generation::ninja_text_for_shell_with_bash(
            &graph,
            graph_generation.recipe_shell,
            graph_generation.bash_executable.as_ref(),
        )
    })
    .context(localization::message(keys::RUNNER_CONTEXT_GENERATE_NINJA))?;
    Ok(GeneratedNinjaWithTargets {
        bundle,
        command_list_targets,
    })
}
