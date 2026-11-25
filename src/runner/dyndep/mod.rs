//! Runner-owned dyndep bundle telemetry and sidecar publication.

pub(in crate::runner) mod generation_telemetry;
mod publication;

pub(in crate::runner) use publication::{
    DyndepPublication, materialize_dyndep_bundle, materialize_dyndep_bundle_in,
    prune_dyndep_bundle, prune_dyndep_bundle_in,
};

/// Publish sidecars into an already-resolved workspace for tests.
///
/// This is the production path with the bundle wrapper removed: it writes
/// through the workspace handle the caller holds, so a test can observe that
/// publication follows that handle rather than the selected path.
#[cfg(test)]
pub(in crate::runner) fn materialize_sidecars_for_test(
    workspace: &super::phony_state::Workspace,
    files: &[crate::ninja_gen::GeneratedDyndep],
) -> anyhow::Result<super::process::DyndepPublicationLease> {
    super::process::materialize_dyndep_files(workspace.dir(), files)
}
