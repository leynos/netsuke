//! Refuse build paths that resolve into Netsuke's reserved generated state.
//!
//! Generation folds a spelling lexically, which is all a pure translation may
//! do. An absolute path, a `..` path that re-enters the workspace, or a
//! symbolic-link alias names generated state only once the filesystem answers
//! what it reaches, so that question is asked here, in the command layer.
//!
//! The selected `--file` manifest is checked with the declared paths. It is not
//! a declared output, so nothing else would refuse a manifest that lands in
//! reserved state, and the reset would then delete the very file the build was
//! asked to read.

use super::resolve::resolves_into_reserved_state;
use super::workspace::Workspace;
use crate::localization::{self, keys};
use anyhow::{Error, Result};
use camino::{Utf8Path, Utf8PathBuf};

/// Proof that a build's paths were refused against reserved generated state.
///
/// Only [`refuse_reserved_declarations`] constructs this value, and the reset
/// requires one, so the destructive reset cannot be reached without the refusal
/// that keeps declared outputs and the selected manifest safe. The value
/// records the workspace it judged and the paths it judged, so the reset can
/// verify that the proof still describes the same inputs.
pub(in crate::runner) struct ReservedStateChecked {
    /// Canonical workspace the paths were resolved against.
    canonical_workspace: Utf8PathBuf,
    /// Every path the refusal judged, in the spelling it judged it.
    checked: Vec<Utf8PathBuf>,
}

impl ReservedStateChecked {
    /// Return the canonical workspace this proof was issued for.
    pub(super) fn canonical_workspace(&self) -> &Utf8Path {
        &self.canonical_workspace
    }

    /// Return every path the refusal judged.
    pub(super) fn checked(&self) -> &[Utf8PathBuf] {
        &self.checked
    }
}

/// Refuse a build whose manifest names a path inside reserved generated state.
///
/// Every declared spelling is resolved against the workspace, so an absolute
/// path, a `..` path that re-enters the workspace, a symbolic-link alias, and a
/// dangling alias are judged as well as the relative spellings generation
/// already rejects. The check runs on every build, before the build removes or
/// creates anything: a manifest that names the force node must not have it
/// deleted, and a manifest that names a hidden output's parent must not have
/// that directory created, because either would collide with generated state
/// the build owns.
///
/// # Errors
///
/// Returns [`ReservedStateChecked`] when every path is safe, and otherwise an
/// error when a path cannot be resolved, or when one resolves into a reserved
/// namespace.
pub(in crate::runner) fn refuse_reserved_declarations(
    workspace: &Workspace,
    manifest: &Utf8Path,
    declared: &[Utf8PathBuf],
) -> Result<ReservedStateChecked> {
    let canonical = workspace.canonical();
    let mut checked = Vec::with_capacity(declared.len() + 1);
    let declared_paths = declared.iter().map(Utf8PathBuf::as_path);
    for path in std::iter::once(manifest).chain(declared_paths) {
        if resolves_into_reserved_state(canonical, path)? {
            return Err(alias_refusal(path));
        }
        checked.push(path.to_path_buf());
    }
    Ok(ReservedStateChecked {
        canonical_workspace: canonical.to_path_buf(),
        checked,
    })
}

/// Verify that a proof still describes `workspace` and its paths.
///
/// The workspace must be the one the refusal judged, and every recorded path
/// must still be outside reserved state. Re-checking is cheap and makes the
/// reset safe even if the filesystem changed between the refusal and the
/// removal.
///
/// # Errors
///
/// Returns an error when the proof belongs to another workspace, when a
/// recorded path cannot be resolved, or when one now resolves into a reserved
/// namespace.
pub(super) fn verify_reservation(
    workspace: &Workspace,
    reservation: &ReservedStateChecked,
) -> Result<()> {
    if reservation.canonical_workspace() != workspace.canonical() {
        return Err(Error::msg(
            "the reservation proof was issued for a different workspace",
        ));
    }
    for path in reservation.checked() {
        if resolves_into_reserved_state(workspace.canonical(), path)? {
            return Err(alias_refusal(path));
        }
    }
    Ok(())
}

/// Build the refusal for a declared path that resolves into reserved state.
///
/// The message is the generator's own reservation diagnostic: this check only
/// extends the same reservation to spellings that need the working directory
/// to resolve, so users see one explanation for one rule.
fn alias_refusal(path: &Utf8Path) -> Error {
    Error::msg(
        localization::message(keys::NINJA_GEN_RESERVED_OUTPUT_PATH)
            .with_arg("path", path.as_str())
            .to_string(),
    )
}
