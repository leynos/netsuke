//! Verify that the force node is cleared only through a real namespace and
//! never under a spelling that resolves to a declared output.
//!
//! The reset runs on every build, so a manifest path that names the force node
//! indirectly — an absolute path, a `..` that returns to the workspace, or a
//! symbolic-link alias — must stop the removal instead of being destroyed by
//! it. The properties below compute their expectation from the workspace
//! itself, so they cannot quietly assume that a path climbing above the working
//! directory is safe.

use super::{
    FORCE_ENTRY, NAMESPACE_LEAF, NAMESPACE_PARENT, open_verified_dir, remove_path,
    reset_force_node, resolves_into_reserved_state,
};
use crate::cli::Cli;
use crate::ninja_gen::FORCE_PATH;
use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use proptest::prelude::*;
use rstest::rstest;
use tempfile::TempDir;

/// A temporary workspace opened as a capability handle.
struct Workspace {
    /// Owns the temporary directory for the test's lifetime.
    _directory: TempDir,
    /// Canonical absolute path of the same directory.
    root: Utf8PathBuf,
    /// Capability handle rooted at [`Self::root`].
    dir: Dir,
}

/// Create a fresh temporary workspace.
fn workspace() -> Result<Workspace> {
    let directory = TempDir::new().context("create a temporary workspace")?;
    let path = Utf8Path::from_path(directory.path()).context("workspace path is UTF-8")?;
    let root = path
        .canonicalize_utf8()
        .context("canonicalize the temporary workspace")?;
    let dir = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open the temporary workspace")?;
    Ok(Workspace {
        _directory: directory,
        root,
        dir,
    })
}

/// Create the phony namespace and return a handle to it.
fn namespace(root: &Dir) -> Result<Dir> {
    root.create_dir_all(Utf8Path::new(NAMESPACE_PARENT).join(NAMESPACE_LEAF))
        .context("create the phony namespace")?;
    open_verified_dir(root, NAMESPACE_PARENT, Utf8Path::new(NAMESPACE_PARENT))?
        .context("the namespace parent must exist")?
        .open_dir(NAMESPACE_LEAF)
        .context("open the phony namespace")
}

/// Write a stray entry at the force node.
fn stray_force_node(root: &Dir) -> Result<()> {
    namespace(root)?
        .write(FORCE_ENTRY, b"stray")
        .context("write a stray force node")
}

/// Run the reset with `root` as the working directory and `declared` as the
/// manifest's paths.
fn reset(root: &Utf8Path, declared: &[Utf8PathBuf]) -> Result<()> {
    let cli = Cli {
        directory: Some(root.to_owned()),
        ..Cli::default()
    };
    reset_force_node(&cli, declared)
}

/// Report whether an entry still occupies the force node.
///
/// The force node's parent is an ancestor of every path inside the namespace,
/// so the verified namespace handle answers for the node itself without
/// following a link that might have replaced it.
fn force_node_exists(dir: &Dir) -> Result<bool> {
    Ok(namespace(dir)?.try_exists(FORCE_ENTRY)?)
}

#[test]
fn the_component_constants_compose_the_force_path() {
    let composed = Utf8Path::new(NAMESPACE_PARENT)
        .join(NAMESPACE_LEAF)
        .join(FORCE_ENTRY);
    assert_eq!(
        composed.as_str(),
        FORCE_PATH,
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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// An absolute path into the namespace is always reserved.
    ///
    /// The oracle is the workspace's own absolute path, so the property holds
    /// regardless of where the temporary directory lives.
    #[test]
    fn absolute_namespace_paths_are_reserved(
        tail in prop::sample::select(vec![
            "phony", "phony/force", "phony/edge-abc", "serial/deadbeef", "dyndep/x.dd",
        ]),
    ) {
        let workspace = workspace().expect("create a temporary workspace");
        let declared = workspace.root.join(format!(".netsuke/{tail}"));
        prop_assert!(resolves_into_reserved_state(&workspace.root, &declared));
    }

    /// A climb that returns to the workspace's own namespace is reserved.
    ///
    /// `d0/d1/../..` lands on the workspace, so the tail names the same node
    /// the literal spelling does. This is the case a check over the graph's
    /// spelling alone cannot see, and it must not be assumed harmless.
    #[test]
    fn a_returning_climb_is_reserved(
        depth in 1usize..4,
        inner in prop::sample::select(vec![
            "phony/force", "phony/edge-abc", "serial/deadbeef", "dyndep/x.dd",
        ]),
    ) {
        let workspace = workspace().expect("create a temporary workspace");
        let prefix = (0..depth)
            .map(|level| format!("d{level}"))
            .collect::<Vec<String>>()
            .join("/");
        let declared = Utf8PathBuf::from(format!(
            "{prefix}/{climb}.netsuke/{inner}",
            climb = "../".repeat(depth),
        ));
        prop_assert!(
            resolves_into_reserved_state(&workspace.root, &declared),
            "{declared} resolves into the workspace namespace",
        );
    }

    /// Climbing above the workspace and back into it is reserved.
    ///
    /// `../<workspace name>/...` names the workspace itself, so the property
    /// that a path climbing above the working directory is safe would be wrong.
    #[test]
    fn a_climb_above_the_root_that_reenters_is_reserved(
        inner in prop::sample::select(vec![
            "phony/force", "phony/edge-abc", "serial/deadbeef", "dyndep/x.dd",
        ]),
    ) {
        let workspace = workspace().expect("create a temporary workspace");
        let name = workspace.root.file_name().expect("workspace has a name");
        let declared = Utf8PathBuf::from(format!("../{name}/.netsuke/{inner}"));
        prop_assert!(
            resolves_into_reserved_state(&workspace.root, &declared),
            "{declared} re-enters the workspace",
        );
    }

    /// A climb that leaves the workspace stays unreserved.
    ///
    /// The expectation is computed from the resolved location, not from the
    /// number of `..` components: only paths landing inside this workspace's
    /// namespaces may be refused.
    #[test]
    fn a_climb_into_another_tree_is_not_reserved(
        name in "[a-z][a-z0-9]{0,6}",
    ) {
        let workspace = workspace().expect("create a temporary workspace");
        let declared = Utf8PathBuf::from(format!("../not-{name}/.netsuke/phony/force"));
        prop_assert!(
            !resolves_into_reserved_state(&workspace.root, &declared),
            "{declared} lands outside the workspace",
        );
    }

    /// Ordinary relative names are never reserved.
    #[test]
    fn ordinary_names_are_not_reserved(
        segments in prop::collection::vec("[a-z][a-z0-9_-]{0,7}", 1..4),
    ) {
        let workspace = workspace().expect("create a temporary workspace");
        let declared = Utf8PathBuf::from(segments.join("/"));
        prop_assert!(!resolves_into_reserved_state(&workspace.root, &declared));
    }
}
