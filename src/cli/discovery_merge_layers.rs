//! File-layer insertion for the cached configuration merge.
//!
//! Keeps merge-specific logging and error accumulation outside discovery while
//! preserving discovery's ownership of the cached layer values.

use ortho_config::MergeComposer;
use std::sync::Arc;

use super::{DiscoveredLayers, diagnostics::short_hash};

/// Add discovered file layers to the supplied merge composition.
///
/// This helper belongs only to the cached merge boundary. It appends to the
/// caller-owned composer and error collection, and never discovers layers or
/// completes a partial merge.
pub(crate) fn push_discovered_file_layers(
    composer: &mut MergeComposer,
    errors: &mut Vec<Arc<ortho_config::OrthoError>>,
    discovered: DiscoveredLayers,
) {
    let (layers, discovery_errors) = discovered.into_parts();
    if discovery_errors.is_empty() {
        tracing::debug!(
            layer = "file",
            layer_count = layers.len(),
            "collected configuration file layers"
        );
    } else {
        tracing::debug!(
            layer = "file",
            error_count = discovery_errors.len(),
            "configuration file layer collection failed"
        );
    }
    errors.extend(discovery_errors);
    for layer in layers {
        tracing::debug!(
            layer = "file",
            path_hash = layer
                .path()
                .map(|path| short_hash(path.as_str().as_bytes())),
            "applied configuration file layer"
        );
        composer.push_layer(layer);
    }
}
