//! Discovery of configuration file layers.
//!
//! Runs the `OrthoConfig` scan and applies the project-scope second pass, so a
//! project `.netsuke.toml` outranks user-scope files. Path comparison and its
//! fallback policy live here because that policy is a discovery decision.

use ortho_config::{ConfigDiscovery, MergeLayer, OrthoResult, load_config_file_as_chain};
use std::borrow::Cow;
use std::path::{Path, PathBuf};

use super::CONFIG_ENV_VAR;
use super::diagnostics::debug_optional_config_path;
use super::paths::{FsPathNormalizer, PathNormalizer, normalized_path_key};

/// Build the single-pass `OrthoConfig` discovery scanner.
///
/// Anchors the project root to `directory` when supplied; otherwise the default
/// project roots apply. `NETSUKE_CONFIG` is registered as the discovery env var.
fn config_discovery(directory: Option<&PathBuf>) -> ConfigDiscovery {
    let mut builder = ConfigDiscovery::builder("netsuke").env_var(CONFIG_ENV_VAR);
    if let Some(dir) = directory {
        builder = builder.clear_project_roots().add_project_root(dir);
    }
    builder.build()
}

pub(crate) fn collect_file_layers(
    directory: Option<&Path>,
) -> OrthoResult<Vec<MergeLayer<'static>>> {
    collect_file_layers_with_normalizer(directory, &FsPathNormalizer)
}

/// Return the key used to compare `path` against the expected project file.
///
/// This is the discovery-side fallback policy for [`normalized_path_key`]. A
/// path that cannot be resolved — most often the expected `.netsuke.toml`, which
/// frequently does not exist, or a directory the process cannot read — is
/// compared literally with `OrthoConfig`'s already-canonicalized layer path
/// rather than failing discovery. An exact textual match still identifies the layer;
/// otherwise the project-scope pass appends it and emits its normal debug event.
fn comparison_key(normalizer: &impl PathNormalizer, path: &str) -> PathBuf {
    normalized_path_key(normalizer, path).unwrap_or_else(|_| PathBuf::from(path))
}

/// Build the discovery layer chain, resolving paths through `normalizer`.
pub(super) fn collect_file_layers_with_normalizer(
    directory: Option<&Path>,
    normalizer: &impl PathNormalizer,
) -> OrthoResult<Vec<MergeLayer<'static>>> {
    let discovery = config_discovery(directory.map(PathBuf::from).as_ref());
    let mut file_layers = discovery.compose_layers();
    let mut errors = file_layers.required_errors;
    if file_layers.value.is_empty() {
        errors.append(&mut file_layers.optional_errors);
    }
    if let Some(err) = errors.into_iter().next() {
        return Err(err);
    }

    let project_file = project_scope_file(directory);
    let project_key = project_file
        .as_deref()
        .map(|path| comparison_key(normalizer, &path.to_string_lossy()));
    let has_project_layer = file_layers.value.iter().any(|layer| {
        layer.path().is_some_and(|path| {
            project_key
                .as_deref()
                .is_some_and(|key| key.to_string_lossy() == path.as_str())
        })
    });
    let project_file_display = project_file.as_deref().map(Path::to_string_lossy);
    if has_project_layer {
        debug_optional_config_path(
            "discovery included project-scope layers",
            project_file_display.as_deref(),
        );
        return Ok(file_layers.value);
    }

    debug_optional_config_path(
        "appending project-scope layers",
        project_file_display.as_deref(),
    );
    let project_layers = project_scope_layers(project_file.as_deref())?;
    Ok(file_layers
        .value
        .into_iter()
        .chain(project_layers)
        .collect())
}

fn project_scope_file(directory: Option<&Path>) -> Option<PathBuf> {
    let root = directory
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())?;
    Some(root.join(".netsuke.toml"))
}

fn project_scope_layers(project_file: Option<&Path>) -> OrthoResult<Vec<MergeLayer<'static>>> {
    let Some(path) = project_file else {
        return Ok(Vec::new());
    };
    match load_config_file_as_chain(path) {
        Ok(Some(chain)) => Ok(chain
            .values
            .into_iter()
            .map(|(value, layer_path)| MergeLayer::file(Cow::Owned(value), Some(layer_path)))
            .collect()),
        Ok(None) => Ok(Vec::new()),
        Err(err) => Err(err),
    }
}
