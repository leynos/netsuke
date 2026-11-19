//! Compose the shared rule name, restat flag and progress description for an edge.
//!
//! This module is the single owner of edge metadata used by ordinary generation
//! and dyndep rendering. Keep its callers limited to those two writers so they
//! emit the same action lookup, progress-label and Ninja-escaping decisions.

use super::escape::{NinjaValue, escape_metadata_value};
use super::force_run::{self, edge_needs_force};
use super::{NinjaGenError, progress_label};
use crate::ir::{BuildEdge, BuildGraph};
use crate::localization::{self, keys};
use camino::Utf8PathBuf;
use ortho_config::Localizer;

/// How one edge's rule is rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EdgeClass {
    /// A dependency-only action, rendered with Ninja's built-in `phony` rule.
    DependencyOnly,
    /// An executable action writing its declared outputs directly.
    Executable,
    /// An executable action whose public names are a native `phony` alias
    /// over a hidden executable output.
    ExecutablePhony,
}

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
    /// Hidden executable output replacing the public names on a phony edge.
    pub(super) hidden_output: Option<Utf8PathBuf>,
    /// Whether the rendered statement depends on the shared force node.
    pub(super) needs_force: bool,
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
    let is_dependency_only = action.recipe.is_dependency_only();
    let class = classify(edge, is_dependency_only);
    let action_name = match class {
        EdgeClass::DependencyOnly => "phony",
        EdgeClass::Executable | EdgeClass::ExecutablePhony => &edge.action_id,
    };
    let localized_description = progress_label::for_edge(edge, action, localizer);
    let description = escape_metadata_value(localized_description.as_deref())?;
    let hidden_output = match class {
        EdgeClass::ExecutablePhony => Some(force_run::hidden_output(edge)),
        EdgeClass::DependencyOnly | EdgeClass::Executable => None,
    };

    Ok(EdgeMetadata {
        action_name,
        restat: action.restat,
        pool: action.pool.as_deref(),
        description,
        hidden_output,
        needs_force: edge_needs_force(edge, is_dependency_only),
    })
}

/// Classify one edge from its recipe shape and its `phony` flag.
///
/// A dependency-only action already renders as Ninja's `phony` rule, so its
/// edge flag cannot add another lowering. The determinism strategy reads the
/// same classification so its coverage flags describe the statements that are
/// actually emitted rather than the raw flags.
pub(super) const fn classify(edge: &BuildEdge, is_dependency_only: bool) -> EdgeClass {
    if is_dependency_only {
        EdgeClass::DependencyOnly
    } else if edge.phony {
        EdgeClass::ExecutablePhony
    } else {
        EdgeClass::Executable
    }
}
