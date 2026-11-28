//! Manifest spellings that only the filesystem can resolve.
//!
//! The selected `--file` manifest is opened by Netsuke itself, so it is judged
//! by filesystem semantics as well as by the lexical form Ninja uses for graph
//! paths. These cases cover the two ways a manifest escapes the lexical check:
//! leaving the workspace through a link under a reserved ancestor, and a `..`
//! that the filesystem applies after following a link.

use super::super::reset::{FORCE_ENTRY, NAMESPACE_LEAF, NAMESPACE_PARENT};
use super::support::{reset_with_manifest, workspace};
use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::TempDir;

/// Refuse a manifest reached through a reserved ancestor.
///
/// The selected spelling is absolute, so no lexical check can see the reserved
/// directory it passes through, and canonicalization follows the link out of
/// the workspace. The reset would still remove `force/` and the link with it,
/// so the ancestor has to decide the question.
#[test]
fn a_manifest_beneath_a_reserved_link_is_refused() -> Result<()> {
    let workspace = workspace()?;
    let outside = TempDir::new().context("create the directory outside the workspace")?;
    let outside_path = Utf8Path::from_path(outside.path())
        .context("the outside path is UTF-8")?
        .to_path_buf();
    let outside_dir = Dir::open_ambient_dir(&outside_path, ambient_authority())
        .context("open the outside directory")?;
    outside_dir
        .write("Netsukefile", b"netsuke_version: '1.0.0'\n")
        .context("write the manifest outside the workspace")?;

    // The link content is relative because the capability API refuses an
    // absolute target; climbing out of the workspace and back reaches the
    // outside directory, so the selected manifest really does live beyond it.
    let name = outside_path
        .file_name()
        .context("the outside directory has a name")?;
    let force = Utf8Path::new(NAMESPACE_PARENT)
        .join(NAMESPACE_LEAF)
        .join(FORCE_ENTRY);
    workspace
        .dir
        .create_dir_all(&force)
        .context("occupy the force node with a directory")?;
    workspace
        .dir
        .symlink(format!("../../../../{name}"), force.join("link"))
        .context("link the force directory outside the workspace")?;

    let selected = workspace.root.join(&force).join("link").join("Netsukefile");
    let outcome = reset_with_manifest(&workspace.root, &selected, &[]);
    ensure!(
        outcome.is_err(),
        "a manifest reached through a reserved ancestor must be refused",
    );
    ensure!(
        outside_dir.exists("Netsukefile"),
        "the refused build must leave the selected manifest in place",
    );
    Ok(())
}

/// Refuse a manifest whose `..` walks through a link on the filesystem.
///
/// Netsuke opens the selected manifest through the filesystem, which follows a
/// link before it applies `..`. Cancelling the spelling first, as Ninja does
/// for graph paths, therefore answers a different question: the cancelled
/// spelling is ordinary while the filesystem spelling lands in the namespace
/// the reset removes.
#[test]
fn a_manifest_dot_dot_after_a_link_is_refused() -> Result<()> {
    let workspace = workspace()?;
    let force = Utf8Path::new(NAMESPACE_PARENT)
        .join(NAMESPACE_LEAF)
        .join(FORCE_ENTRY);
    workspace
        .dir
        .create_dir_all(force.join("sub"))
        .context("occupy the force node with a directory")?;
    workspace
        .dir
        .write(force.join("Netsukefile"), b"netsuke_version: '1.0.0'\n")
        .context("write the manifest inside the reserved directory")?;
    workspace
        .dir
        .symlink(
            Utf8Path::new(NAMESPACE_PARENT)
                .join(NAMESPACE_LEAF)
                .join(FORCE_ENTRY)
                .join("sub"),
            "link",
        )
        .context("link a workspace name into the reserved directory")?;

    let selected = Utf8PathBuf::from("link/../Netsukefile");
    let outcome = reset_with_manifest(&workspace.root, &selected, &[]);
    ensure!(
        outcome.is_err(),
        "a filesystem `..` through a link into reserved state must be refused",
    );
    ensure!(
        workspace.dir.exists(force.join("Netsukefile")),
        "the refused build must leave the selected manifest in place",
    );
    Ok(())
}

/// Keep publication on the workspace the refusal judged.
///
/// A `-C` path can be a symbolic link. If publication reopened the selected
/// path after the refusal, retargeting that link in between would publish
/// sidecars into a directory whose reserved namespace was never judged. The
/// workspace is resolved once, and every later step uses that value's handle,
/// so retargeting the link afterwards must not move publication.
#[cfg(unix)]
#[test]
fn retargeting_the_selected_directory_does_not_move_publication() -> Result<()> {
    let chosen = TempDir::new().context("create the first directory")?;
    let other = TempDir::new().context("create the second directory")?;
    let link_root = TempDir::new().context("create the link directory")?;
    let chosen_path = Utf8Path::from_path(chosen.path())
        .context("the first path is UTF-8")?
        .to_path_buf();
    let other_path = Utf8Path::from_path(other.path())
        .context("the second path is UTF-8")?
        .to_path_buf();
    let link_path = Utf8Path::from_path(link_root.path())
        .context("the link path is UTF-8")?
        .to_path_buf();
    let selected = link_path.join("selected");
    let link_dir = Dir::open_ambient_dir(&link_path, ambient_authority())
        .context("open the link directory")?;
    // The capability API refuses an absolute link target, so the link climbs
    // one level and names the sibling temporary directory.
    let chosen_name = chosen_path
        .file_name()
        .context("the first directory has a name")?;
    let other_name = other_path
        .file_name()
        .context("the second directory has a name")?;
    link_dir
        .symlink(Utf8Path::new("..").join(chosen_name), "selected")
        .context("link the selected directory to the first workspace")?;

    // Resolve once, as a build does before it judges any path.
    let workspace = super::super::Workspace::open(&selected)?;

    // Retarget the link afterwards, as a concurrent change could.
    link_dir
        .remove_file("selected")
        .context("remove the original link")?;
    link_dir
        .symlink(Utf8Path::new("..").join(other_name), "selected")
        .context("retarget the link to the second workspace")?;

    let sidecar = crate::ninja_gen::GeneratedDyndep::fixture(
        Utf8PathBuf::from(".netsuke/dyndep/fixture.dd"),
        "ninja_dyndep_version = 1\n".to_owned(),
    );
    let _publication = super::super::super::dyndep::materialize_sidecars_for_test(
        &workspace,
        std::slice::from_ref(&sidecar),
    )?;

    ensure!(
        chosen_path.join(".netsuke/dyndep/fixture.dd").exists(),
        "publication must write into the workspace the refusal judged",
    );
    ensure!(
        !other_path.join(".netsuke").exists(),
        "publication must not follow the retargeted link",
    );
    Ok(())
}
