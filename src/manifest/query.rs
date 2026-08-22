//! Workspace-backed manifest loading for builds and discovery queries.
//!
//! This module owns the capability-scoped filesystem boundary shared by normal
//! manifest loading and `netsuke help targets`. The latter selects a restricted
//! stdlib registration so it can render discovery metadata without allowing
//! network requests, cache writes, or command execution.

use super::{
    EnvReader, ManifestLoadStage, ManifestName, ManifestParse, NetsukeManifest, StdlibConfig,
    StdlibRegistration, env_reader::disabled_env_reader, from_str_named, notify_stage,
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
pub(crate) fn from_path_for_manifest_query(
    path: impl AsRef<Path>,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    let env_reader = disabled_env_reader();
    from_path_with_registration(path, &env_reader, on_stage, ManifestLoadMode::ManifestQuery)
}

/// Load a manifest with the full stdlib and an explicit network policy.
pub(super) fn from_path_with_policy_and_env(
    path: impl AsRef<Path>,
    policy: NetworkPolicy,
    env_reader: &EnvReader,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    from_path_with_registration(path, env_reader, on_stage, ManifestLoadMode::Full(policy))
}

/// Select the standard-library boundary for a manifest load.
enum ManifestLoadMode {
    /// A normal build load with a configured network policy.
    Full(NetworkPolicy),
    /// A metadata-only load that must not construct an ambient stdlib config.
    ManifestQuery,
}

/// Read a manifest and render it with the selected stdlib registration.
fn from_path_with_registration(
    path: impl AsRef<Path>,
    env_reader: &EnvReader,
    mut on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
    mode: ManifestLoadMode,
) -> Result<NetsukeManifest> {
    notify_stage(&mut on_stage, ManifestLoadStage::ManifestIngestion);
    let path_ref = path.as_ref();
    let workspace = open_manifest_workspace(path_ref, None)?;
    let data = workspace
        .dir
        .read_to_string(&workspace.manifest_file)
        .with_context(|| {
            localization::message(keys::MANIFEST_READ_FAILED)
                .with_arg("path", path_ref.display().to_string())
        })?;
    let name = ManifestName::new(path_ref.display().to_string());
    let manifest_root = Some(workspace.root.clone().into_std_path_buf());
    let stdlib_registration = match mode {
        ManifestLoadMode::Full(policy) => StdlibRegistration::Full(Box::new(
            StdlibConfig::new(workspace.dir)?
                .with_workspace_root_path(workspace.root)?
                .with_network_policy(policy),
        )),
        ManifestLoadMode::ManifestQuery => StdlibRegistration::ManifestQuery,
    };
    from_str_named(
        &data,
        ManifestParse {
            name: &name,
            stdlib_registration: Some(stdlib_registration),
            env_reader,
            manifest_root,
        },
        &mut on_stage,
    )
}
