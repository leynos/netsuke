//! File-path manifest loaders and their compatibility adapters.
//!
//! Every entry point here loads a manifest from disk and delegates the
//! workspace-backed read to the manifest query loader. The adapters differ only
//! in how much of the environment, resource-ceiling, and stage-observer seam the
//! caller supplies, so they are grouped together and re-exported from the
//! parent module.

use super::{
    EnvAccessPolicy, EnvReader, ManifestBudgetLimits, ManifestEnvironment, ManifestLoadStage,
    process_env_reader, query,
};
use crate::{ast::NetsukeManifest, stdlib::NetworkPolicy};
use anyhow::Result;
use std::path::Path;

/// Load a [`NetsukeManifest`] from the given file path.
///
/// # Errors
///
/// Returns an error if the file cannot be read, the template cannot be
/// rendered, or the YAML cannot be parsed.
pub fn from_path(path: impl AsRef<Path>) -> Result<NetsukeManifest> {
    from_path_with_policy(path, NetworkPolicy::default(), None)
}

/// Load a [`NetsukeManifest`] with an explicit network policy.
///
/// Invoke `on_stage` in order for each manifest-loading stage when it is set.
///
/// # Errors
///
/// Return an error when the file cannot be read or rendered.
///
/// # Examples
///
/// ```rust,ignore
/// use netsuke::manifest;
/// use netsuke::stdlib::NetworkPolicy;
///
/// let policy = NetworkPolicy::default();
/// let manifest = manifest::from_path_with_policy("Netsukefile", policy, None);
/// assert!(manifest.is_ok());
/// ```
pub fn from_path_with_policy(
    path: impl AsRef<Path>,
    policy: NetworkPolicy,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    from_path_with_policy_and_limits(path, policy, ManifestBudgetLimits::default(), on_stage)
}

/// Load a manifest with explicit network policy and resource ceilings.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, rendered, or parsed.
pub fn from_path_with_policy_and_limits(
    path: impl AsRef<Path>,
    policy: NetworkPolicy,
    budget_limits: ManifestBudgetLimits,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    from_path_with_policy_and_env_and_limits(
        path,
        policy,
        &process_env_reader(),
        budget_limits,
        on_stage,
    )
}

/// Load a manifest with explicit network policy and an environment reader.
///
/// This adapter boundary lets callers supply deterministic manifest variables
/// without mutating the process environment. Its access policy is permissive
/// for compatibility; callers needing access control should use
/// [`from_path_with_policy_and_environment`].
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, rendered, or parsed.
///
/// # Examples
///
/// ```
/// use netsuke::{ast::Recipe, manifest::{EnvReadError, EnvReader,
///     from_path_with_policy_and_env}, stdlib::NetworkPolicy};
/// use std::{io::Write, sync::Arc};
///
/// let mut file = tempfile::NamedTempFile::new().expect("create manifest");
/// write!(
///     file,
///     "netsuke_version: 1.0.0\ntargets:\n  - name: build\n    command: echo {{{{ env('PROFILE') }}}}\n"
/// )
/// .expect("write manifest");
/// let reader: EnvReader = Arc::new(|name| match name {
///     "PROFILE" => Ok("offline".to_owned()),
///     _ => Err(EnvReadError::NotPresent),
/// });
/// let policy = NetworkPolicy::default().deny_all_hosts();
/// let manifest = from_path_with_policy_and_env(file.path(), policy, &reader, None)
///     .expect("load manifest without network access");
///
/// assert!(matches!(
///     &manifest.targets[0].recipe,
///     Recipe::Command { command } if command.as_single() == Some("echo offline")
/// ));
/// ```
pub fn from_path_with_policy_and_env(
    path: impl AsRef<Path>,
    policy: NetworkPolicy,
    env_reader: &EnvReader,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    from_path_with_policy_and_env_and_limits(
        path,
        policy,
        env_reader,
        ManifestBudgetLimits::default(),
        on_stage,
    )
}

/// Load a manifest with explicit network policy, an environment reader, and
/// resource ceilings.
///
/// The access policy is permissive for compatibility, matching
/// [`from_path_with_policy_and_env`]; callers needing access control should use
/// [`from_path_with_policy_and_environment_and_limits`].
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, rendered, or parsed.
#[expect(
    clippy::too_many_arguments,
    reason = "This compatibility entry point keeps the policy, environment, budget, and stage-observer seams explicit."
)]
pub fn from_path_with_policy_and_env_and_limits(
    path: impl AsRef<Path>,
    policy: NetworkPolicy,
    env_reader: &EnvReader,
    budget_limits: ManifestBudgetLimits,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    let environment = ManifestEnvironment::new(env_reader, EnvAccessPolicy::default());
    from_path_with_policy_and_environment_and_limits(
        path,
        policy,
        &environment,
        budget_limits,
        on_stage,
    )
}

/// Load a manifest with explicit network policy and environment inputs.
///
/// This adapter boundary keeps an injected reader paired with the policy that
/// governs every Jinja `env()` lookup.
///
/// # Examples
/// ```rust,ignore
/// let _ = from_path_with_policy_and_environment("Netsukefile", policy, &environment, None);
/// ```
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, rendered, or parsed.
pub fn from_path_with_policy_and_environment(
    path: impl AsRef<Path>,
    policy: NetworkPolicy,
    environment: &ManifestEnvironment<'_>,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    from_path_with_policy_and_environment_and_limits(
        path,
        policy,
        environment,
        ManifestBudgetLimits::default(),
        on_stage,
    )
}

/// Load a manifest with explicit policy, environment inputs, and resource
/// ceilings.
///
/// This is the fullest-parameterized loader entry point: an injected reader
/// and its access policy, plus the parse ceilings trusted configuration
/// resolved before loading.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, rendered, or parsed.
#[expect(
    clippy::too_many_arguments,
    reason = "This compatibility entry point keeps the policy, environment, budget, and stage-observer seams explicit."
)]
pub fn from_path_with_policy_and_environment_and_limits(
    path: impl AsRef<Path>,
    policy: NetworkPolicy,
    environment: &ManifestEnvironment<'_>,
    budget_limits: ManifestBudgetLimits,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    query::from_path_with_policy_and_environment_and_limits(
        path,
        policy,
        environment,
        budget_limits,
        on_stage,
    )
}
