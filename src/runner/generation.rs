//! Pure Ninja-generation steps for the runner.
//!
//! Generation decomposes into three query-style steps — load the manifest,
//! build the graph, generate the Ninja text — none of which require a status
//! reporter. Progress reporting stays in the thin orchestration wrappers in
//! [`super`] (`generate_ninja`,
//! `load_manifest_with_stage_reporting`), so generation can be reused as a
//! pure operation (for example for dry runs or background generation).

use anyhow::{Context, Result};
use camino::Utf8Path;

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
/// # Examples
///
/// ```rust,ignore
/// let manifest = load_manifest(
///     Utf8Path::new("Netsukefile"),
///     NetworkPolicy::default(),
///     None,
/// )?;
/// // `manifest` is rendered and ready for `build_graph`.
/// ```
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
/// # Examples
///
/// ```rust,ignore
/// let graph = build_graph(&manifest)?;
/// // `graph` contains the validated targets and actions for `ninja_text`.
/// ```
///
/// # Errors
///
/// Returns an error when graph construction or validation fails (for example
/// on circular dependencies or duplicate outputs).
pub(super) fn build_graph(manifest: &NetsukeManifest) -> Result<BuildGraph> {
    BuildGraph::from_manifest(manifest)
        .context(localization::message(keys::RUNNER_CONTEXT_BUILD_GRAPH))
}

/// Generate the Ninja bundle for a build graph.
///
/// # Examples
///
/// ```rust,ignore
/// let generated = ninja_text(&graph)?;
/// let (text, sidecars) = generated.into_parts();
/// assert!(text.contains("build hello:"));
/// assert!(sidecars.is_empty());
/// ```
///
/// # Errors
///
/// Returns an error when Ninja synthesis fails.
pub(super) fn ninja_text(
    graph: &BuildGraph,
) -> Result<ninja_gen::GeneratedNinja, ninja_gen::NinjaGenError> {
    ninja_gen::generate_bundle(graph)
}
