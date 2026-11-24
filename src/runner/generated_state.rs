//! Sequence the build steps that touch generated state.
//!
//! The reservation refusal has run before this module is reached. What remains
//! is publication, pruning, and the force-node reset, in that order, with the
//! publication lease held until Ninja exits. Keeping the sequence here means
//! `execute_build` calls one named step instead of relying on comments to keep
//! three side-effecting operations in order.

use super::dyndep::{DyndepPublication, materialize_dyndep_bundle, prune_dyndep_bundle};
use super::phony_state::{ResetOutcome, Workspace, refuse_reserved_declarations, reset_force_node};
use crate::cli::Cli;
use crate::ninja_gen::GeneratedNinja;
use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};

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
    plan: &BuildStatePlan<'_>,
) -> Result<DyndepPublication> {
    let reservation = refuse_reserved_declarations(plan.workspace, plan.manifest, plan.declared)?;
    let publication = materialize_dyndep_bundle(cli, plan.bundle)?;
    prune_dyndep_bundle(cli, plan.bundle.dyndep_files(), &publication)?;
    report_reset(reset_force_node(plan.workspace, &reservation)?);
    Ok(publication)
}

/// The inputs one build's generated-state step judges and publishes.
pub(super) struct BuildStatePlan<'a> {
    /// Working directory, resolved once for the refusal and the reset.
    pub(super) workspace: &'a Workspace,
    /// Generated bundle whose sidecars are published and pruned.
    pub(super) bundle: &'a GeneratedNinja,
    /// Selected manifest, judged with the declared paths.
    pub(super) manifest: &'a Utf8Path,
    /// Every path the graph named, in the graph's own spelling.
    pub(super) declared: &'a [Utf8PathBuf],
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
