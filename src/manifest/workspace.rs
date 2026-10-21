//! Capability-scoped access to the directory containing a manifest.
//!
//! Opening the workspace through `cap_std` keeps later reads inside the
//! manifest's own directory tree; the ambient authority is exercised exactly
//! once, here, to obtain the handle.

use crate::localization::{self, keys};
use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use std::{env, path::Path};

/// Resolve a potentially relative manifest parent path to an absolute UTF-8 workspace root.
///
/// An injected `base` directory anchors relative parents for tests; `None`
/// falls back to the process current directory so production behaviour is
/// unchanged.
pub(super) fn resolve_absolute_workspace_root(
    utf8_parent: &Utf8Path,
    base: Option<&Path>,
) -> Result<Utf8PathBuf> {
    let workspace_base = if utf8_parent.is_absolute() {
        utf8_parent.to_path_buf().into_std_path_buf()
    } else {
        let anchor = match base {
            // A relative base (for example `Path::new(".")`) is anchored at the
            // process working directory so the workspace root keeps its
            // documented "absolute UTF-8 path" contract even for test bases.
            Some(dir) if dir.is_absolute() => dir.to_path_buf(),
            Some(dir) => env::current_dir()
                .context(localization::message(keys::MANIFEST_RESOLVE_WORKSPACE_ROOT))?
                .join(dir),
            None => env::current_dir()
                .context(localization::message(keys::MANIFEST_RESOLVE_WORKSPACE_ROOT))?,
        };
        anchor.join(utf8_parent.as_std_path())
    };
    Utf8PathBuf::from_path_buf(workspace_base).map_err(|invalid| {
        anyhow!(
            "{}",
            localization::message(keys::MANIFEST_WORKSPACE_NON_UTF8)
                .with_arg("path", invalid.display().to_string())
        )
    })
}

/// A manifest's workspace opened for capability-based access.
#[derive(Debug)]
pub(super) struct ManifestWorkspace {
    /// Capability-scoped handle on the workspace root.
    pub(super) dir: Dir,
    /// Absolute UTF-8 path of the workspace root.
    pub(super) root: Utf8PathBuf,
    /// Manifest file name relative to the workspace root.
    pub(super) manifest_file: String,
}

/// Open the directory containing `path` as a capability-scoped workspace.
///
/// `base` anchors relative manifest paths for tests; `None` keeps the ambient
/// current-directory resolution used by production callers.
pub(super) fn open_manifest_workspace(
    path: &Path,
    base: Option<&Path>,
) -> Result<ManifestWorkspace> {
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let manifest_file = match path.file_name() {
        None => {
            return Err(anyhow!(
                "{}",
                localization::message(keys::MANIFEST_PATH_MISSING_NAME)
                    .with_arg("path", path.display().to_string())
            ));
        }
        Some(name) => name.to_str().map(str::to_owned).ok_or_else(|| {
            anyhow!(
                "{}",
                localization::message(keys::MANIFEST_PATH_NON_UTF8)
                    .with_arg("manifest", path.display().to_string())
                    .with_arg("path", path.display().to_string())
            )
        })?,
    };
    let utf8_parent = Utf8Path::from_path(parent).ok_or_else(|| {
        anyhow!(
            "{}",
            localization::message(keys::MANIFEST_PATH_NON_UTF8)
                .with_arg("manifest", &manifest_file)
                .with_arg("path", path.display().to_string())
        )
    })?;
    let root = resolve_absolute_workspace_root(utf8_parent, base)?;
    tracing::debug!(workspace = %root, manifest = %manifest_file, "opening manifest workspace directory");
    let dir = Dir::open_ambient_dir(root.as_path(), ambient_authority())
        .inspect_err(|err| {
            tracing::warn!(workspace = %root, manifest = %manifest_file, error = %err, "failed to open manifest workspace directory");
        })
        .with_context(|| {
            localization::message(keys::MANIFEST_OPEN_WORKSPACE_FAILED)
                .with_arg("workspace", root.as_str())
                .with_arg("manifest", &manifest_file)
        })?;
    Ok(ManifestWorkspace {
        dir,
        root,
        manifest_file,
    })
}
