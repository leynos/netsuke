//! Properties over spellings the working directory, not the graph, decides.
//!
//! Each case computes its expectation from a real workspace, so no property
//! encodes the assumption that a path climbing above the working directory is
//! safe: a climb can leave and return, and the oracle follows it.

use super::super::resolve::resolves_into_reserved_state;
use super::support::workspace;
use camino::Utf8PathBuf;
use proptest::prelude::*;

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
        prop_assert!(
            resolves_into_reserved_state(&workspace.root, &declared)
                .expect("resolve the declared path"),
        );
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
            resolves_into_reserved_state(&workspace.root, &declared)
                .expect("resolve the declared path"),
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
            resolves_into_reserved_state(&workspace.root, &declared)
                .expect("resolve the declared path"),
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
            !resolves_into_reserved_state(&workspace.root, &declared)
                .expect("resolve the declared path"),
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
        prop_assert!(
            !resolves_into_reserved_state(&workspace.root, &declared)
                .expect("resolve the declared path"),
        );
    }
}
