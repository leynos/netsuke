//! Workspace-backed manifest loading for builds and discovery queries.
//!
//! This module owns the capability-scoped filesystem boundary shared by normal
//! manifest loading and `netsuke help targets`. The caller supplies the
//! [`StdlibRegistration`] each load needs — a discovery query selects the
//! restricted surface — so this module only reads, names, and hands the
//! manifest to the string loader.

use super::{
    ManifestBudgetLimits, ManifestLoadOptions, ManifestLoadStage, ManifestName, NetsukeManifest,
    StdlibRegistration,
    budget::adapter::from_str_named,
    workspace::{ManifestWorkspace, open_manifest_workspace},
};
use crate::{localization, localization::keys};
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
    from_path_for_manifest_query_with_source(path, budget_limits, on_stage)
        .map(|loaded| loaded.manifest)
}

/// Load a manifest for a discovery query, keeping the source text it came from.
///
/// The linter needs the exact bytes the manifest was parsed from so its span
/// index cannot disagree with the compiler's. Returning them from the same
/// capability-scoped read is what guarantees that; reading the path again
/// would open a second, unscoped view of the filesystem.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, rendered, or parsed, if it
/// exceeds `budget_limits`, or if it invokes an impure template helper.
pub(crate) fn from_path_for_manifest_query_with_source(
    path: impl AsRef<Path>,
    budget_limits: ManifestBudgetLimits,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<LoadedManifest> {
    let mut options = ManifestLoadOptions::manifest_query().with_budget_limits(budget_limits);
    if let Some(observer) = on_stage {
        options = options.with_stage_observer(observer);
    }
    from_path_with_options(
        path,
        |_workspace| Ok(StdlibRegistration::ManifestQuery),
        options,
    )
}

/// Read one manifest through the capability-scoped workspace boundary.
///
/// `registration_for` derives the load's capabilities from the opened
/// workspace. That is what lets a path load configure the stdlib against the
/// directory the manifest actually lives in; string loads have no workspace, so
/// they decide their registration up front instead.
pub(super) fn from_path_with_options(
    path: impl AsRef<Path>,
    registration_for: impl FnOnce(ManifestWorkspace) -> Result<StdlibRegistration>,
    mut options: ManifestLoadOptions<'_>,
) -> Result<LoadedManifest> {
    let manifest_path = path.as_ref();
    options.notify(ManifestLoadStage::ManifestIngestion);
    let workspace = open_manifest_workspace(manifest_path, None)?;
    let data = workspace
        .dir
        .read_to_string(&workspace.manifest_file)
        .with_context(|| {
            localization::message(keys::MANIFEST_READ_FAILED)
                .with_arg("path", manifest_path.display().to_string())
        })?;
    let name = ManifestName::new(manifest_path.display().to_string());
    // Clone the root before the closure consumes the workspace: the directory
    // handle moves into `StdlibConfig`, which is not cloneable.
    let manifest_root = Some(workspace.root.clone());
    let registration = registration_for(workspace)?;
    let manifest = from_str_named(
        &data,
        options
            .with_registration(registration)
            .into_parse(&name, manifest_root),
    )?;
    Ok(LoadedManifest {
        manifest,
        #[cfg(feature = "lint")]
        source: data,
        #[cfg(feature = "lint")]
        name: name.as_ref().to_owned(),
    })
}

/// A rendered manifest together with the source text it was parsed from.
///
/// The source and its display name are kept only for the linter's span index;
/// a build without the `lint` feature has no reader for them.
pub(crate) struct LoadedManifest {
    /// The expanded and rendered manifest.
    pub(crate) manifest: NetsukeManifest,
    /// The manifest source, exactly as read.
    #[cfg(feature = "lint")]
    pub(crate) source: String,
    /// The display name the diagnostics label the source with.
    #[cfg(feature = "lint")]
    pub(crate) name: String,
}
