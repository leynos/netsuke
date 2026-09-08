//! Workspace-backed manifest loading for builds and discovery queries.
//!
//! This module owns the capability-scoped filesystem boundary shared by normal
//! manifest loading and `netsuke help targets`. The latter selects a restricted
//! stdlib registration so it can render discovery metadata without allowing
//! network requests, cache writes, or command execution.

use super::{
    EnvReader, ExpansionReportObserver, ManifestBudgetLimits, ManifestLoadStage, ManifestName,
    ManifestParse, NetsukeManifest, StdlibConfig, StdlibRegistration,
    env_reader::disabled_env_reader, from_str_named, loading::trace_expansion_report, notify_stage,
    workspace::open_manifest_workspace,
};
use crate::{localization, localization::keys, stdlib::NetworkPolicy};
use anyhow::{Context, Result};
use std::path::Path;

/// Load a manifest for a side-effect-free discovery query.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, rendered, or parsed, or if
/// it invokes an impure template helper.
#[cfg(test)]
pub(crate) fn from_path_for_manifest_query(
    path: impl AsRef<Path>,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    from_path_for_manifest_query_with_limits(path, ManifestBudgetLimits::default(), on_stage)
}

/// Load a manifest query with caller-controlled resource ceilings.
pub(crate) fn from_path_for_manifest_query_with_limits(
    path: impl AsRef<Path>,
    budget_limits: ManifestBudgetLimits,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    let env_reader = disabled_env_reader();
    from_path_with_registration(ManifestLoadRequest {
        path: path.as_ref(),
        env_reader: &env_reader,
        budget_limits,
        on_stage,
        mode: ManifestLoadMode::ManifestQuery,
    })
}

/// Load a full manifest with explicit policy and resource ceilings.
#[expect(
    clippy::too_many_arguments,
    reason = "This compatibility entry point keeps the established policy, environment, budget, and stage-observer seams explicit."
)]
pub(super) fn from_path_with_policy_and_env_and_limits(
    path: impl AsRef<Path>,
    policy: NetworkPolicy,
    env_reader: &EnvReader,
    budget_limits: ManifestBudgetLimits,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    from_path_with_registration(ManifestLoadRequest {
        path: path.as_ref(),
        env_reader,
        budget_limits,
        on_stage,
        mode: ManifestLoadMode::Full(policy),
    })
}

/// Select the standard-library boundary for a manifest load.
enum ManifestLoadMode {
    /// A normal build load with a configured network policy.
    Full(NetworkPolicy),
    /// A metadata-only load that must not construct an ambient stdlib config.
    ManifestQuery,
}

/// Hold the dependencies needed to load one manifest from a workspace path.
struct ManifestLoadRequest<'path, 'env, 'stage> {
    /// Identifies the manifest file on the capability-scoped workspace boundary.
    path: &'path Path,
    /// Reads environment variables through the caller-selected boundary.
    env_reader: &'env EnvReader,
    /// Supplies immutable resource ceilings for this parse.
    budget_limits: ManifestBudgetLimits,
    /// Observes load phases without changing parsing behaviour.
    on_stage: Option<&'stage mut dyn FnMut(ManifestLoadStage)>,
    /// Selects full or metadata-only standard-library registration.
    mode: ManifestLoadMode,
}

/// Read a manifest and render it with the selected stdlib registration.
fn from_path_with_registration(
    mut request: ManifestLoadRequest<'_, '_, '_>,
) -> Result<NetsukeManifest> {
    notify_stage(&mut request.on_stage, ManifestLoadStage::ManifestIngestion);
    let workspace = open_manifest_workspace(request.path, None)?;
    let data = workspace
        .dir
        .read_to_string(&workspace.manifest_file)
        .with_context(|| {
            localization::message(keys::MANIFEST_READ_FAILED)
                .with_arg("path", request.path.display().to_string())
        })?;
    let name = ManifestName::new(request.path.display().to_string());
    let (stdlib_registration, expansion_report_observer): (
        StdlibRegistration,
        Option<ExpansionReportObserver>,
    ) = match request.mode {
        ManifestLoadMode::Full(policy) => (
            StdlibRegistration::Full(Box::new(
                StdlibConfig::new(workspace.dir)?
                    .with_workspace_root_path(&workspace.root)?
                    .with_network_policy(policy),
            )),
            Some(trace_expansion_report),
        ),
        ManifestLoadMode::ManifestQuery => (StdlibRegistration::ManifestQuery, None),
    };
    let manifest_root = Some(workspace.root);
    from_str_named(
        &data,
        ManifestParse {
            name: &name,
            stdlib_registration: Some(stdlib_registration),
            env_reader: request.env_reader,
            manifest_root,
            expansion_report_observer,
            budget_limits: request.budget_limits,
        },
        &mut request.on_stage,
    )
}
