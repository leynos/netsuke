//! Compose the shared rule name, restat flag and progress description for an edge.
//!
//! This module is the single owner of edge metadata used by ordinary generation
//! and dyndep rendering. Keep its callers limited to those two writers so they
//! emit the same action lookup, progress-label and Ninja-escaping decisions.

use super::escape::{NinjaValue, escape_metadata_value};
use super::{NinjaGenError, progress_label};
use crate::ir::{BuildEdge, BuildGraph};
use crate::localization::{self, keys};
use ortho_config::Localizer;

/// Carry resolved metadata from either edge writer to `DisplayEdge`.
pub(super) struct EdgeMetadata<'a> {
    /// Ninja rule name, or `phony` for a dependency-only action.
    pub(super) action_name: &'a str,
    /// Whether Ninja should remove the output before running the recipe.
    pub(super) restat: bool,
    /// Pool selected by the shared action, if any.
    pub(super) pool: Option<&'a str>,
    /// Optional, validated edge-level description with Ninja dollars escaped.
    pub(super) description: Option<NinjaValue>,
}

/// Resolve and escape the metadata shared by ordinary and dyndep edge writers.
///
/// Resolve the action once, preserve the existing missing-action error, and
/// compose the progress label with the localizer chosen for this generation.
///
/// # Examples
///
/// For an inline edge named `check-fmt` with `restat` disabled, the result
/// carries `action_name = "check-fmt"`, `restat = false` and the description
/// `Some("Building \u{2068}check-fmt\u{2069}")` under the English localizer.
///
/// # Errors
///
/// Return [`NinjaGenError::MissingAction`] if the edge refers to an absent
/// action, or the existing unsafe-metadata error if its description contains a
/// disallowed Ninja control character.
pub(super) fn for_edge<'a>(
    graph: &'a BuildGraph,
    edge: &'a BuildEdge,
    localizer: &dyn Localizer,
) -> Result<EdgeMetadata<'a>, NinjaGenError> {
    let action =
        graph
            .actions
            .get(&edge.action_id)
            .ok_or_else(|| NinjaGenError::MissingAction {
                id: edge.action_id.clone(),
                message: localization::message(keys::NINJA_GEN_MISSING_ACTION)
                    .with_arg("id", &edge.action_id),
            })?;
    let action_name = if action.recipe.is_dependency_only() {
        "phony"
    } else {
        &edge.action_id
    };
    let localized_description = progress_label::for_edge(edge, action, localizer);
    let description = escape_metadata_value(localized_description.as_deref())?;

    Ok(EdgeMetadata {
        action_name,
        restat: action.restat,
        pool: action.pool.as_deref(),
        description,
    })
}
