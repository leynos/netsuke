//! Verify that the force node is cleared only through a real namespace and
//! never under a spelling that resolves to a declared output.
//!
//! The reset runs on every build, so a manifest path that names the force node
//! indirectly — an absolute path, a `..` that returns to the workspace, or a
//! symbolic-link alias — must stop the removal instead of being destroyed by
//! it. The example cases live here; the properties in [`properties`] compute
//! their expectation from the workspace itself, so they cannot quietly assume
//! that a path climbing above the working directory is safe.

mod properties;
mod support;

use super::{
    FORCE_ENTRY, NAMESPACE_LEAF, NAMESPACE_PARENT, open_verified_dir, remove_path,
    resolves_into_reserved_state,
};
use crate::ninja_gen::FORCE_PATH;
use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::rstest;
use support::{force_node_exists, namespace, reset, stray_force_node, workspace};

#[cfg(unix)]
use cap_primitives::fs::open_dir_nofollow;
#[cfg(unix)]
use std::path::Path;
// Only the Unix-only outside-link case creates a directory of its own.
#[cfg(unix)]
use tempfile::TempDir;

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

#[rstest]
#[case(".netsuke/phony/force", true)]
#[case("out/all", false)]
fn a_relative_spelling_is_judged_against_the_workspace(
    #[case] path: &str,
    #[case] refused: bool,
) -> Result<()> {
    let workspace = workspace()?;
    ensure!(
        resolves_into_reserved_state(&workspace.root, Utf8Path::new(path)) == refused,
        "reservation of {path} must be {refused}",
    );
    Ok(())
}

#[test]
fn an_absolute_alias_stops_the_reset() -> Result<()> {
    let workspace = workspace()?;
    stray_force_node(&workspace.dir)?;
    let declared = vec![workspace.root.join(FORCE_PATH)];
    let outcome = reset(&workspace.root, &declared);
    ensure!(
        outcome.is_err(),
        "an absolute alias must not delete the node it names",
    );
    ensure!(
        force_node_exists(&workspace.dir)?,
        "the refused reset must leave the declared output in place",
    );
    Ok(())
}

#[test]
fn a_parent_relative_alias_that_reenters_stops_the_reset() -> Result<()> {
    let workspace = workspace()?;
    let inner = workspace.root.join("inner");
    workspace
        .dir
        .create_dir_all(
            Utf8Path::new("inner")
                .join(NAMESPACE_PARENT)
                .join(NAMESPACE_LEAF),
        )
        .context("create the inner workspace's namespace")?;
    let inner_dir =
        Dir::open_ambient_dir(&inner, ambient_authority()).context("open the inner workspace")?;
    stray_force_node(&inner_dir)?;

    // `inner` is the working directory, so `../inner` climbs out of it and
    // returns to the same place: only resolution against the working directory
    // can tell that the spelling names the force node.
    let inner_root = inner.canonicalize_utf8().context("canonicalize inner")?;
    let declared = vec![Utf8PathBuf::from("../inner").join(FORCE_PATH)];
    let outcome = reset(&inner_root, &declared);
    ensure!(
        outcome.is_err(),
        "a path that climbs out and back in must not delete its target",
    );
    ensure!(
        force_node_exists(&inner_dir)?,
        "the refused reset must leave the declared output in place",
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_symlinked_alias_stops_the_reset() -> Result<()> {
    let workspace = workspace()?;
    stray_force_node(&workspace.dir)?;
    workspace
        .dir
        .symlink(
            Utf8Path::new(NAMESPACE_PARENT).join(NAMESPACE_LEAF),
            "state",
        )
        .context("link an alias to the namespace")?;
    let declared = vec![Utf8PathBuf::from("state").join(FORCE_ENTRY)];
    let outcome = reset(&workspace.root, &declared);
    ensure!(
        outcome.is_err(),
        "a symbolic-link alias must not delete the node it names",
    );
    ensure!(
        force_node_exists(&workspace.dir)?,
        "the refused reset must leave the declared output in place",
    );
    Ok(())
}

#[test]
fn an_unrelated_declared_path_allows_the_reset() -> Result<()> {
    let workspace = workspace()?;
    stray_force_node(&workspace.dir)?;
    let declared = vec![Utf8PathBuf::from("out/all")];
    reset(&workspace.root, &declared)?;
    ensure!(
        !force_node_exists(&workspace.dir)?,
        "an unrelated manifest path must not stop the reset",
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

/// An absolute declaration that is itself a link out of the workspace is
/// refused.
///
/// Following the final component would describe the link's target instead of
/// the entry that occupies the force node, so the parent is resolved and the
/// entry name kept as written. Deleting here would destroy an output the
/// manifest declared.
#[cfg(unix)]
#[test]
fn an_absolute_alias_through_a_link_stops_the_reset() -> Result<()> {
    let workspace = workspace()?;
    let outside = TempDir::new().context("create a directory outside the workspace")?;
    let outside_path = Utf8Path::from_path(outside.path()).context("the outside path is UTF-8")?;
    let outside_dir = Dir::open_ambient_dir(outside_path, ambient_authority())
        .context("open the outside directory")?;
    outside_dir
        .write("kept", b"outside")
        .context("write the link target")?;

    // The link content is relative because the capability API refuses an
    // absolute target; climbing three levels leaves the workspace and the
    // target exists, so following the final component really does land
    // outside it.
    let name = outside_path
        .file_name()
        .context("the outside directory has a name")?;
    let target = format!("../../../{name}/kept");
    namespace(&workspace.dir)?
        .symlink(&target, FORCE_ENTRY)
        .context("link the force node outside the workspace")?;
    let declared = vec![workspace.root.join(FORCE_PATH)];
    let outcome = reset(&workspace.root, &declared);
    ensure!(
        outcome.is_err(),
        "a declared link out of the workspace must stop the reset",
    );
    ensure!(
        force_node_exists(&workspace.dir)?,
        "the refused reset must leave the declared entry in place",
    );
    Ok(())
}
