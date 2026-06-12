//! Pure Ninja-generation steps for the runner.
//!
//! Generation decomposes into three query-style steps — load the manifest,
//! build the graph, generate the Ninja text — none of which require a
//! [`crate::status::StatusReporter`]. Progress reporting stays in the thin
//! orchestration wrappers in [`super`] (`generate_ninja`,
//! `load_manifest_with_stage_reporting`), so generation can be reused as a
//! pure operation (for example for dry runs or background generation).

use anyhow::{Context, Result};
use camino::Utf8Path;

use super::NinjaContent;
use crate::ast::NetsukeManifest;
use crate::ir::BuildGraph;
use crate::localization::{self, keys};
use crate::stdlib::NetworkPolicy;
use crate::{manifest, ninja_gen};

/// Optional observer for manifest-loading stages.
///
/// Callers that want progress reporting pass a callback translating
/// [`manifest::ManifestLoadStage`] values into their own reporting; passing
/// `None` keeps the pipeline free of side effects.
pub(super) type StageObserver<'a> = Option<&'a mut dyn FnMut(manifest::ManifestLoadStage)>;

/// Load and render the Netsuke manifest at `path`.
///
/// # Errors
///
/// Returns an error when the manifest cannot be read, parsed, or rendered.
pub(super) fn load_manifest(
    path: &Utf8Path,
    policy: NetworkPolicy,
    on_stage: StageObserver<'_>,
) -> Result<NetsukeManifest> {
    manifest::from_path_with_policy(path.as_std_path(), policy, on_stage).with_context(|| {
        localization::message(keys::RUNNER_CONTEXT_LOAD_MANIFEST).with_arg("path", path.as_str())
    })
}

/// Translate a manifest into the build graph intermediate representation.
///
/// # Errors
///
/// Returns an error when graph construction or validation fails (for example
/// on circular dependencies or duplicate outputs).
pub(super) fn build_graph(manifest: &NetsukeManifest) -> Result<BuildGraph> {
    BuildGraph::from_manifest(manifest)
        .context(localization::message(keys::RUNNER_CONTEXT_BUILD_GRAPH))
}

/// Generate the Ninja manifest text for a build graph.
///
/// # Errors
///
/// Returns an error when Ninja synthesis fails.
pub(super) fn ninja_text(graph: &BuildGraph) -> Result<NinjaContent> {
    ninja_gen::generate(graph)
        .map(NinjaContent::new)
        .context(localization::message(keys::RUNNER_CONTEXT_GENERATE_NINJA))
}
