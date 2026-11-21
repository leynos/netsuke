//! Which graph spellings the pure reservation check folds and answers.
//!
//! These cases cover only what the graph's own text decides. Spellings that
//! need the working directory — absolute paths and `..` climbs that may return
//! to it — belong to the command layer's tests, where the workspace is known,
//! so they appear here as explicit non-answers rather than as assumptions.

use super::is_reserved_path;
use camino::{Utf8Path, Utf8PathBuf};
use proptest::prelude::*;
use rstest::rstest;

#[rstest]
#[case(".netsuke/phony", true)]
#[case(".netsuke/phony/force", true)]
#[case(".netsuke/phony/edge-abc", true)]
#[case(".netsuke/serial/deadbeef", true)]
#[case(".netsuke/dyndep/deadbeef.dd", true)]
#[case("./.netsuke/phony/force", true)]
#[case("out/../.netsuke/phony/force", true)]
#[case("out/./../.netsuke/serial/x", true)]
#[case(".netsuke/phony/.", true)]
#[case(".NETSUKE/PHONY/force", true)]
#[case(".Netsuke/Dyndep/x.dd", true)]
#[case("./.NETSUKE/Phony/Force", true)]
#[case(".netsuke/phony/..", false)]
#[case(".netsuke", false)]
#[case(".netsuke/phoney/force", false)]
#[case(".netsuke/phonyx", false)]
#[case("out/.netsuke/phony", false)]
#[case("phony/force", false)]
#[case("out/all", false)]
fn the_pure_check_folds_the_graph_spelling(#[case] path: &str, #[case] expected: bool) {
    assert_eq!(
        is_reserved_path(Utf8Path::new(path)),
        expected,
        "pure reservation of {path} must be {expected}"
    );
}

#[rstest]
#[case("../.netsuke/phony/force")]
#[case("/workspace/.netsuke/phony/force")]
fn spellings_that_need_the_working_directory_are_not_decided_alone(#[case] path: &str) {
    assert!(
        !is_reserved_path(Utf8Path::new(path)),
        "the graph spelling of {path} alone must not decide reservation"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Adding `.` components to a spelling cannot change its reservation.
    ///
    /// A spelling variant a manifest author might write must reach the same
    /// answer as the literal, whatever the path names.
    #[test]
    fn cur_dir_prefixes_do_not_change_the_answer(
        segments in prop::collection::vec("[a-zA-Z0-9_.-]{1,6}", 0..4)
    ) {
        let path = Utf8PathBuf::from(segments.join("/"));
        let prefixed = Utf8PathBuf::from(format!("./{path}"));
        prop_assert_eq!(is_reserved_path(&path), is_reserved_path(&prefixed));
    }

    /// A `..` preceded by a component stays foldable and stays reserved.
    ///
    /// `prefix/../.netsuke/...` resolves to the namespace whenever `prefix`
    /// names one directory, so folding must not lose the namespace.
    #[test]
    fn a_foldable_climb_keeps_the_namespace(
        prefix in "[a-z][a-z0-9]{0,5}",
        suffix in prop::sample::select(vec![
            "phony", "phony/force", "phony/edge-abc", "serial/deadbeef", "dyndep/x.dd",
        ]),
    ) {
        let path = Utf8PathBuf::from(format!("{prefix}/../.netsuke/{suffix}"));
        prop_assert!(is_reserved_path(&path), "{path} must stay reserved");
    }

    /// Names outside the reserved namespaces are never reserved.
    #[test]
    fn plain_names_are_never_reserved(
        segments in prop::collection::vec("[a-zA-Z0-9_-]{1,8}", 1..4)
    ) {
        let path = Utf8PathBuf::from(segments.join("/"));
        prop_assert!(!is_reserved_path(&path), "{path} must not be reserved");
    }

    /// Appending a filename inside a namespace keeps it reserved.
    ///
    /// The strategy starts with a name character, so it never produces the
    /// navigation components `.` and `..`; those are not descendants and have
    /// their own cases in the table above.
    #[test]
    fn every_descendant_of_a_namespace_is_reserved(
        namespace in prop::sample::select(vec![".netsuke/phony", ".netsuke/serial", ".netsuke/dyndep"]),
        leaf in "[a-zA-Z0-9_][a-zA-Z0-9_.-]{0,11}",
    ) {
        let path = Utf8PathBuf::from(format!("{namespace}/{leaf}"));
        prop_assert!(is_reserved_path(&path), "{path} must be reserved");
    }
}
