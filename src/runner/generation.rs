//! Query-style Ninja-generation steps and the build manifest loader.
//!
//! Generation decomposes into three query-style steps — load the manifest,
//! build the graph, generate the Ninja text — none of which require a status
//! reporter. Progress reporting stays in the thin orchestration wrappers in
//! [`super`] (`generate_ninja`,
//! `load_manifest_with_stage_reporting`), so generation can be reused as a
//! pure operation (for example for dry runs or background generation).
//!
//! [`load_manifest`] is the read-only query step: it rejects template helpers
//! that can access the environment, filesystem, network, clock, or shell.
//! [`load_manifest_for_build_with_limits`] is deliberately separate because
//! command execution needs the full, effectful manifest stdlib.

use anyhow::{Context, Result};
use camino::Utf8Path;

use crate::ast::NetsukeManifest;
use crate::cli::Cli;
use crate::ir::{BuildGraph, IrGenError};
use crate::localization::{self, keys};
use crate::{manifest, ninja_gen};
use crate::{
    manifest::{EnvAccessPolicy, ManifestEnvironment},
    stdlib::NetworkPolicy,
};

/// Optional observer for manifest-loading stages.
///
/// Callers that want progress reporting pass a callback translating
/// [`manifest::ManifestLoadStage`] values into their own reporting; passing
/// `None` keeps the pipeline free of side effects.
pub(super) type StageObserver<'a> = Option<&'a mut dyn FnMut(manifest::ManifestLoadStage)>;

/// Trusted configuration bounding one build manifest load.
///
/// Bundles the network grant, environment grant, and resource ceilings that
/// trusted configuration resolved before loading. Grouping them keeps the
/// loader seams explicit while holding the parameter list to the workspace
/// ceiling, as `manifest::ManifestParse` does for the parse path.
pub(crate) struct ManifestLoadInputs {
    /// Network grant ceiling applied to fetch helpers.
    pub(super) network_policy: NetworkPolicy,
    /// Environment grant ceiling evaluated before each `env()` read.
    pub(super) env_access_policy: EnvAccessPolicy,
    /// Resource ceilings applied to manifest evaluation.
    pub(super) budget_limits: manifest::ManifestBudgetLimits,
}

impl ManifestLoadInputs {
    /// Resolve the trusted configuration bounding one build manifest load.
    ///
    /// # Errors
    ///
    /// Returns an error when the merged network policy or the merged resource
    /// ceilings are invalid.
    pub(super) fn from_cli(cli: &Cli) -> Result<Self> {
        Ok(Self {
            network_policy: cli
                .network_policy()
                .context(localization::message(keys::RUNNER_CONTEXT_NETWORK_POLICY))?,
            env_access_policy: cli.env_access_policy(),
            budget_limits: cli.manifest_budget_limits()?,
        })
    }
}

/// Load and render the Netsuke manifest at `path` without effectful helpers.
///
/// # Examples
///
/// ```rust,ignore
/// let manifest = load_manifest(Utf8Path::new("Netsukefile"), None)?;
/// // `manifest` is rendered and ready for `build_graph`.
/// ```
///
/// # Errors
///
/// Returns an error when the manifest cannot be read, parsed, or rendered.
#[cfg(test)]
pub(super) fn load_manifest(
    path: &Utf8Path,
    on_stage: StageObserver<'_>,
) -> Result<NetsukeManifest> {
    load_manifest_with_limits(path, manifest::ManifestBudgetLimits::default(), on_stage)
}

/// Load a manifest query with explicit resource ceilings.
pub(super) fn load_manifest_with_limits(
    path: &Utf8Path,
    budget_limits: manifest::ManifestBudgetLimits,
    on_stage: StageObserver<'_>,
) -> Result<NetsukeManifest> {
    manifest::from_path_for_manifest_query_with_limits(path.as_std_path(), budget_limits, on_stage)
        .with_context(|| {
            localization::message(keys::RUNNER_CONTEXT_LOAD_MANIFEST)
                .with_arg("path", path.as_str())
        })
}

/// Load and render a manifest with the full, effectful build stdlib.
///
/// This loader is only for command execution. Templates may use configured
/// network, cache, environment, filesystem, clock, and shell helpers.
///
/// # Examples
///
/// ```rust,ignore
/// let inputs = ManifestLoadInputs {
///     network_policy: NetworkPolicy::default(),
///     env_access_policy: EnvAccessPolicy::default(),
///     budget_limits: manifest::ManifestBudgetLimits::default(),
/// };
/// let manifest = load_manifest_for_build_with_limits(
///     Utf8Path::new("Netsukefile"),
///     &inputs,
///     None,
/// )?;
/// // `manifest` may use build-time template helpers before `build_graph`.
/// ```
///
/// The access policy is evaluated before the process environment is read, so a
/// blocked `env()` call cannot disclose a host value.
///
/// # Errors
///
/// Returns an error when the manifest cannot be read, parsed, or rendered.
pub(super) fn load_manifest_for_build_with_limits(
    path: &Utf8Path,
    inputs: &ManifestLoadInputs,
    on_stage: StageObserver<'_>,
) -> Result<NetsukeManifest> {
    let env_reader = manifest::process_env_reader();
    let environment = ManifestEnvironment::new(&env_reader, inputs.env_access_policy.clone());
    manifest::from_path_with_policy_and_environment_and_limits(
        path.as_std_path(),
        inputs.network_policy.clone(),
        &environment,
        inputs.budget_limits,
        on_stage,
    )
    .with_context(|| {
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

/// Translate a manifest into a graph for one legacy recipe interpreter.
///
/// # Errors
///
/// Returns an error when graph construction or validation fails (for example
/// on circular dependencies or duplicate outputs).
pub(super) fn build_graph_for_shell(
    manifest: &NetsukeManifest,
    shell: crate::recipe_shell::RecipeShell,
) -> std::result::Result<BuildGraph, IrGenError> {
    BuildGraph::from_manifest_for_shell(manifest, shell)
}

/// Generate the Ninja bundle for a build graph.
///
/// # Examples
///
/// ```rust,ignore
/// let generated = ninja_text_for_shell(&graph, RecipeShell::host_default())?;
/// let (text, sidecars) = generated.into_parts();
/// assert!(text.contains("build hello:"));
/// assert!(sidecars.is_empty());
/// ```
///
/// Generate Ninja text using the selected legacy-recipe interpreter.
///
/// # Errors
///
/// Returns an error when Ninja synthesis fails.
pub(super) fn ninja_text_for_shell(
    graph: &BuildGraph,
    shell: crate::recipe_shell::RecipeShell,
) -> Result<ninja_gen::GeneratedNinja, ninja_gen::NinjaGenError> {
    ninja_gen::dyndep::generate_bundle_for_shell(graph, shell)
}
