//! Sequence the build steps that touch generated state.
//!
//! The reservation refusal has run before this module is reached. What remains
//! is publication, pruning, and the force-node reset, in that order, with the
//! publication lease held until Ninja exits. Keeping the sequence here means
//! `execute_build` calls one named step instead of relying on comments to keep
//! three side-effecting operations in order.

use super::dyndep::{DyndepPublication, materialize_dyndep_bundle, prune_dyndep_bundle};
use super::phony_state::{ReservedStateChecked, ResetOutcome, Workspace, reset_force_node};
use crate::cli::Cli;
use crate::ninja_gen::GeneratedNinja;
use anyhow::Result;

/// Publish the dyndep bundle, prune it, then clear the force node.
///
/// Publication comes first because Ninja reads the sidecars, pruning because
/// obsolete ones are bounded, and the reset last because it removes state the
/// earlier steps do not read. The reset still takes the refusal's own proof, so
/// moving this call cannot move the destructive step ahead of the check.
///
/// # Errors
///
/// Returns an error when the bundle cannot be published or pruned, or when the
/// force node cannot be reset.
pub(super) fn publish_then_reset(
    cli: &Cli,
    workspace: &Workspace,
    bundle: &GeneratedNinja,
    reservation: &ReservedStateChecked,
) -> Result<DyndepPublication> {
    let publication = materialize_dyndep_bundle(cli, bundle)?;
    prune_dyndep_bundle(cli, bundle.dyndep_files(), &publication)?;
    report_reset(reset_force_node(workspace, reservation)?);
    Ok(publication)
}

/// Report a reset that removed a stray force node.
///
/// A clear node is the normal case and needs no event; a removal means a stray
/// path had switched forcing off, so it is worth a warning.
fn report_reset(outcome: ResetOutcome) {
    if let ResetOutcome::Removed { entry } = outcome {
        tracing::warn!(
            path = %format!(".netsuke/phony/{entry}"),
            "removed a stray path from the reserved phony namespace; forced edges stay dirty"
        );
    }
}
