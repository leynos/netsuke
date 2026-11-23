//! Verify that the force node is cleared only through a real namespace.
//!
//! The reset runs on every build, so `.netsuke` and its `phony` leaf must be
//! opened as real directories without following a link, and only what occupies
//! `force` beneath them may be removed. The example cases here cover those
//! handles and the removal itself; [`resolution`] covers the declared
//! spellings that need the working directory to judge, and [`properties`]
//! computes their expectation from the workspace rather than assuming that a
//! path climbing above the working directory is safe.

#[cfg(unix)]
mod manifest_spellings;
mod properties;
mod resolution;
mod support;

use super::reset::{FORCE_ENTRY, NAMESPACE_LEAF, NAMESPACE_PARENT, open_verified_dir, remove_path};
use super::{Workspace as BuildWorkspace, refuse_reserved_declarations, reset_force_node};
use crate::ninja_gen::FORCE_PATH;
use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use support::{namespace, workspace};

#[cfg(unix)]
use cap_primitives::fs::open_dir_nofollow;
#[cfg(unix)]
use std::path::Path;

#[test]
fn the_component_constants_compose_the_force_path() {
    // Compose the slash-separated spelling directly. A joined path would carry
    // the platform separator, which the generator never writes.
    let composed = format!("{NAMESPACE_PARENT}/{NAMESPACE_LEAF}/{FORCE_ENTRY}");
    assert_eq!(
        composed, FORCE_PATH,
        "the reset must clear the node the generator names"
    );
}

#[test]
fn an_absent_namespace_is_left_absent() -> Result<()> {
    let workspace = workspace()?;
    let opened = open_verified_dir(
        &workspace.dir,
        NAMESPACE_PARENT,
        Utf8Path::new(NAMESPACE_PARENT),
    )?;
    ensure!(
        opened.is_none(),
        "an absent namespace holds no state to clear",
    );
    Ok(())
}

#[test]
fn a_regular_file_at_the_namespace_is_refused() -> Result<()> {
    let workspace = workspace()?;
    workspace
        .dir
        .write(NAMESPACE_PARENT, b"not a directory")
        .context("write a file where the namespace belongs")?;
    let refused = open_verified_dir(
        &workspace.dir,
        NAMESPACE_PARENT,
        Utf8Path::new(NAMESPACE_PARENT),
    );
    ensure!(refused.is_err(), "a file cannot be the reserved namespace");
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_symlinked_namespace_is_refused() -> Result<()> {
    let workspace = workspace()?;
    workspace
        .dir
        .create_dir_all("data")
        .context("create a link target")?;
    workspace
        .dir
        .symlink("data", NAMESPACE_PARENT)
        .context("link the namespace to another directory")?;
    let refused = open_verified_dir(
        &workspace.dir,
        NAMESPACE_PARENT,
        Utf8Path::new(NAMESPACE_PARENT),
    );
    ensure!(
        refused.is_err(),
        "a symlink could point outside the reserved namespace",
    );
    Ok(())
}

#[test]
fn an_absent_force_node_is_left_absent() -> Result<()> {
    let workspace = workspace()?;
    let phony = namespace(&workspace.dir)?;
    remove_path(&phony, Utf8Path::new(FORCE_ENTRY))?;
    ensure!(
        !phony
            .try_exists(FORCE_ENTRY)
            .context("check the force node")?,
        "resetting an absent force node must not create it",
    );
    Ok(())
}

#[test]
fn a_regular_file_at_the_force_node_is_removed() -> Result<()> {
    let workspace = workspace()?;
    let phony = namespace(&workspace.dir)?;
    phony
        .write(FORCE_ENTRY, b"stray")
        .context("write a stray force node")?;
    remove_path(&phony, Utf8Path::new(FORCE_ENTRY))?;
    ensure!(
        !phony
            .try_exists(FORCE_ENTRY)
            .context("check the force node")?,
        "a stray file at the force node must be removed",
    );
    Ok(())
}

#[test]
fn a_directory_at_the_force_node_is_removed_with_its_contents() -> Result<()> {
    let workspace = workspace()?;
    let phony = namespace(&workspace.dir)?;
    phony
        .create_dir_all(FORCE_ENTRY)
        .context("create a stray directory at the force node")?;
    phony
        .write(Utf8PathBuf::from(FORCE_ENTRY).join("keep"), b"stray")
        .context("write inside the stray directory")?;
    remove_path(&phony, Utf8Path::new(FORCE_ENTRY))?;
    ensure!(
        !phony
            .try_exists(FORCE_ENTRY)
            .context("check the force node")?,
        "a stray directory at the force node must be removed",
    );
    Ok(())
}

/// The verified open refuses a link as the final component.
///
/// The reset's safety against a link swapped in after the inspection rests on
/// this guarantee from the capability primitive, not on the inspection itself.
/// Pinning the primitive here means a dependency change that dropped the
/// refusal fails this test rather than silently weakening every reset.
#[cfg(unix)]
#[test]
fn the_namespace_open_primitive_refuses_a_link() -> Result<()> {
    let workspace = workspace()?;
    workspace
        .dir
        .create_dir("real")
        .context("create a link target")?;
    workspace
        .dir
        .symlink("real", "state")
        .context("link a directory alias")?;
    let handle = workspace
        .dir
        .as_cap_std()
        .try_clone()
        .context("clone the workspace handle")?
        .into_std_file();
    let outcome = open_dir_nofollow(&handle, Path::new("state"));
    ensure!(
        outcome.is_err(),
        "the verified open must refuse a link as its final component",
    );
    Ok(())
}

/// A link at the `phony` leaf is refused, not followed.
///
/// Both namespace components go through the same verified open, so the leaf
/// needs its own case: `.netsuke` may be real while `phony` is the link.
#[cfg(unix)]
#[test]
fn a_symlinked_phony_leaf_is_refused() -> Result<()> {
    let workspace = workspace()?;
    let display = Utf8Path::new(NAMESPACE_PARENT).join(NAMESPACE_LEAF);
    workspace
        .dir
        .create_dir_all(Utf8Path::new(NAMESPACE_PARENT).join("data"))
        .context("create a link target")?;
    workspace
        .dir
        .symlink("data", &display)
        .context("link the phony leaf to another directory")?;
    let netsuke = open_verified_dir(
        &workspace.dir,
        NAMESPACE_PARENT,
        Utf8Path::new(NAMESPACE_PARENT),
    )?
    .context("the namespace parent must exist")?;
    let refused = open_verified_dir(&netsuke, NAMESPACE_LEAF, &display);
    ensure!(
        refused.is_err(),
        "a linked phony leaf could point outside the reserved namespace",
    );
    Ok(())
}

/// Refuse a reservation proof issued for another workspace.
///
/// The proof records the workspace it judged, so a caller cannot reuse one
/// workspace's refusal to authorise a reset somewhere else. Without the check,
/// the removal would run against a directory the refusal never saw.
#[test]
fn a_reservation_for_one_workspace_cannot_authorise_another() -> Result<()> {
    let first = workspace()?;
    let second = workspace()?;
    support::stray_force_node(&second.dir)?;
    let first_build = BuildWorkspace::open(&first.root)?;
    let second_build = BuildWorkspace::open(&second.root)?;
    let reservation =
        refuse_reserved_declarations(&first_build, Utf8Path::new("Netsukefile"), &[])?;
    let outcome = reset_force_node(&second_build, &reservation);
    ensure!(
        outcome.is_err(),
        "a proof from another workspace must be refused",
    );
    ensure!(
        support::force_node_exists(&second.dir)?,
        "the refused reset must leave the force node in place",
    );
    Ok(())
}
