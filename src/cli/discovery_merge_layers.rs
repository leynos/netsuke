//! File-layer insertion for the cached configuration merge.
//!
//! Keeps merge-specific logging and error accumulation outside discovery while
//! preserving discovery's ownership of the cached layer values.

use ortho_config::MergeComposer;
use std::sync::Arc;

use super::{DiscoveredLayers, diagnostics::short_hash};
use crate::cli::{MergeEvent, MergeObserver};

/// Add discovered file layers to the supplied merge composition.
///
/// This helper belongs only to the cached merge boundary. It appends to the
/// caller-owned composer and error collection, and never discovers layers or
/// completes a partial merge.
pub(crate) fn push_discovered_file_layers<O>(
    composer: &mut MergeComposer,
    errors: &mut Vec<Arc<ortho_config::OrthoError>>,
    discovered: DiscoveredLayers,
    observer: &mut O,
) where
    O: MergeObserver + ?Sized,
{
    let (layers, discovery_errors) = discovered.into_parts();
    if discovery_errors.is_empty() {
        observer.observe(MergeEvent::FileLayersCollected {
            layer_count: layers.len(),
        });
    } else {
        observer.observe(MergeEvent::FileLayerCollectionFailed {
            error_count: discovery_errors.len(),
        });
    }
    errors.extend(discovery_errors);
    for layer in layers {
        observer.observe(MergeEvent::FileLayerApplied {
            path_hash: layer
                .path()
                .map(|path| short_hash(path.as_str().as_bytes())),
        });
        composer.push_layer(layer);
    }
}
