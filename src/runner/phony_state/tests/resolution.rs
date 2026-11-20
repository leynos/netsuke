//! Example cases for spellings only the working directory can judge.
//!
//! Each case declares the force node through a different route — an absolute
//! path, a `..` that leaves and returns, a symbolic link, or a link that
//! leaves the workspace — and expects the reset to refuse rather than delete
//! the output the manifest named. The cases also cover the two ways the guard
//! could fail quietly: treating an unresolvable path as unrelated, and
//! creating state while deciding.

use super::super::{FORCE_ENTRY, NAMESPACE_LEAF, NAMESPACE_PARENT, resolves_into_reserved_state};
use super::support::{force_node_exists, namespace, reset, stray_force_node, workspace};
use crate::ninja_gen::FORCE_PATH;
use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::rstest;

#[cfg(unix)]
use tempfile::TempDir;

#[rstest]
#[case(".netsuke/phony/force", true)]
#[case("out/all", false)]
fn a_relative_spelling_is_judged_against_the_workspace(
    #[case] path: &str,
    #[case] refused: bool,
) -> Result<()> {
    let workspace = workspace()?;
    let reserved = resolves_into_reserved_state(&workspace.root, Utf8Path::new(path))
        .context("resolve the declared path")?;
    ensure!(
        reserved == refused,
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

/// A declared path that cannot be resolved stops the reset.
///
/// Folding a canonicalization failure into "not reserved" would let the guard
/// fail open, so an unresolvable directory must refuse the removal rather than
/// pass as unrelated. A symbolic-link loop makes resolution fail with
/// something other than "absent", which is the distinction the guard now
/// preserves.
#[cfg(unix)]
#[test]
fn an_unresolvable_alias_stops_the_reset() -> Result<()> {
    let workspace = workspace()?;
    stray_force_node(&workspace.dir)?;
    workspace
        .dir
        .symlink("loop-b", "loop-a")
        .context("link the first half of a loop")?;
    workspace
        .dir
        .symlink("loop-a", "loop-b")
        .context("link the second half of a loop")?;
    let declared = vec![Utf8PathBuf::from("loop-a").join(FORCE_PATH)];
    let outcome = reset(&workspace.root, &declared);
    ensure!(
        outcome.is_err(),
        "a path that cannot be resolved must stop the reset",
    );
    ensure!(
        force_node_exists(&workspace.dir)?,
        "the refused reset must leave the declared output in place",
    );
    Ok(())
}

/// Resolving a declared path reads the workspace without changing it.
///
/// The guard runs before a deletion, so creating state while deciding would
/// hide a write inside a query and could itself make the force node look
/// present.
#[test]
fn resolution_reads_without_creating_state() -> Result<()> {
    let workspace = workspace()?;
    let declared = Utf8PathBuf::from("out/all");
    ensure!(
        !resolves_into_reserved_state(&workspace.root, &declared)
            .context("resolve the declared path")?,
        "an ordinary output is not reserved",
    );
    ensure!(
        !workspace
            .dir
            .try_exists(NAMESPACE_PARENT)
            .context("check the namespace")?,
        "resolving a declared path must not create the namespace",
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
