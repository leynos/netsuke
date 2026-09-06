//! File-layer insertion for the cached configuration merge.
//!
//! Keeps merge-specific logging and error accumulation outside discovery while
//! preserving discovery's ownership of the cached layer values.

use ortho_config::MergeComposer;
use std::sync::Arc;

use super::{DiscoveredLayers, ProjectFetchPolicyRequest, diagnostics::short_hash};
use crate::cli::MergeEvent;

/// Add discovered file layers to the supplied merge composition.
///
/// This helper belongs only to the cached merge boundary. It appends to the
/// caller-owned composer and error collection, and never discovers layers or
/// completes a partial merge.
pub(crate) fn push_discovered_file_layers(
    composer: &mut MergeComposer,
    errors: &mut Vec<Arc<ortho_config::OrthoError>>,
    discovered: DiscoveredLayers,
    events: &mut Vec<MergeEvent>,
) -> Option<ProjectFetchPolicyRequest> {
    let (layers, discovery_errors, project_fetch_policy_request) = discovered.into_parts();
    if discovery_errors.is_empty() {
        events.push(MergeEvent::FileLayersCollected {
            layer_count: layers.len(),
        });
    } else {
        events.push(MergeEvent::FileLayerCollectionFailed {
            error_count: discovery_errors.len(),
        });
        // Keep the original validation error instead of asking the generic
        // schema to deserialize the same malformed field a second time.
        errors.extend(discovery_errors);
        return project_fetch_policy_request;
    }
    for layer in layers {
        events.push(MergeEvent::FileLayerApplied {
            path_hash: layer
                .path()
                .map(|path| short_hash(path.as_str().as_bytes())),
        });
        composer.push_layer(layer);
    }
    project_fetch_policy_request
}
