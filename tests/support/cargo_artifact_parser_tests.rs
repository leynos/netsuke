//! Property and example tests for the Cargo artefact parser.

use super::{compiler_artifact_paths, dependency_dirs_in_message, library_path_in_message};
use proptest::prelude::*;
use rstest::rstest;
use std::path::{Path, PathBuf};

/// Generate a newline-free Cargo artefact path and whether rustc can load it.
fn artefact_path() -> impl Strategy<Value = (String, bool)> {
    (
        "[\\p{L}\\p{N} _-]{1,12}",
        "[\\p{L}\\p{N}_-]{1,12}",
        prop_oneof![
            Just("rmeta"),
            Just("rlib"),
            Just(std::env::consts::DLL_EXTENSION),
            Just("txt")
        ],
    )
        .prop_map(|(directory, name, extension)| {
            let path = format!("/tmp/{directory}/lib{name}.{extension}");
            let loadable = matches!(extension, "rmeta" | "rlib")
                || extension.eq_ignore_ascii_case(std::env::consts::DLL_EXTENSION);
            (path, loadable)
        })
}

/// Generate an ordered path whose extension can select a library artefact.
fn library_artefact_path() -> impl Strategy<Value = String> {
    (
        "[a-z]{1,12}",
        prop_oneof![Just("rmeta"), Just("rlib"), Just("txt"), Just("dylib")],
    )
        .prop_map(|(name, extension)| format!("/tmp/build/lib{name}.{extension}"))
}

/// Derive the metadata-first library selection from an ordered path list.
fn expected_library_path(paths: &[String]) -> Option<PathBuf> {
    ["rmeta", "rlib"].into_iter().find_map(|extension| {
        paths
            .iter()
            .rfind(|path| {
                Path::new(path)
                    .extension()
                    .is_some_and(|value| value == extension)
            })
            .map(PathBuf::from)
    })
}

proptest! {
    /// Preserve every ordered parent directory rustc needs across varied paths.
    #[test]
    fn parser_preserves_loadable_artefact_parent_order(
        artefacts in proptest::collection::vec(artefact_path(), 0..16),
    ) {
        let filenames: Vec<&str> = artefacts.iter().map(|(path, _)| path.as_str()).collect();
        let message = serde_json::json!({
            "reason": "compiler-artifact",
            "target": {"name": "fixture"},
            "filenames": filenames,
        });
        let expected: Vec<PathBuf> = artefacts
            .iter()
            .filter(|(_, loadable)| *loadable)
            .filter_map(|(path, _)| Path::new(path).parent().map(Path::to_path_buf))
            .collect();

        prop_assert_eq!(dependency_dirs_in_message(&message.to_string()), expected);
    }

    /// Select the last metadata artefact, or the last library artefact when needed.
    #[test]
    fn library_parser_prefers_last_metadata_then_library_and_rejects_mismatches(
        artefacts in proptest::collection::vec(library_artefact_path(), 0..16),
    ) {
        let message = serde_json::json!({
            "reason": "compiler-artifact",
            "target": {"name": "fixture"},
            "filenames": &artefacts,
        });
        let expected = expected_library_path(&artefacts);

        prop_assert_eq!(library_path_in_message(&message.to_string(), "fixture"), expected);
        prop_assert_eq!(library_path_in_message(&message.to_string(), "other"), None);
    }
}

/// Prefer metadata and fall back to the library artefact.
#[rstest]
#[case(
    r#"{"reason":"compiler-artifact","target":{"name":"fixture"},"filenames":["/final/libfixture.rlib","/build/libfixture.rmeta"]}"#,
    "/build/libfixture.rmeta",
    "metadata"
)]
#[case(
    r#"{"reason":"compiler-artifact","target":{"name":"fixture"},"filenames":["/final/libfixture.rlib"]}"#,
    "/final/libfixture.rlib",
    "library fallback"
)]
#[case(
    r#"{"reason":"compiler-artifact","target":{"name":"fixture"},"filenames":["/first/libfixture.rmeta","/first/libfixture.rlib","/last/libfixture.rmeta","/last/libfixture.rlib"]}"#,
    "/last/libfixture.rmeta",
    "last metadata"
)]
#[case(
    r#"{"reason":"compiler-artifact","target":{"name":"fixture"},"filenames":["/first/libfixture.rlib","/last/libfixture.rlib"]}"#,
    "/last/libfixture.rlib",
    "last library fallback"
)]
fn parser_prefers_metadata_then_falls_back_to_library(
    #[case] message: &str,
    #[case] expected: &str,
    #[case] selection: &str,
) {
    assert_eq!(
        library_path_in_message(message, "fixture"),
        Some(PathBuf::from(expected)),
        "{selection} should be selected when Cargo reports it"
    );
}

/// Reject malformed and irrelevant messages before reading artefact paths.
#[rstest]
#[case("not JSON")]
#[case(r#"{"reason":"build-script-executed"}"#)]
#[case(r#"{"reason":"compiler-artifact","filenames":[]}"#)]
#[case(r#"{"reason":"compiler-artifact","target":{},"filenames":[]}"#)]
#[case(r#"{"reason":"compiler-artifact","target":{"name":"fixture"}}"#)]
#[case(r#"{"reason":"compiler-artifact","target":{"name":"fixture"},"filenames":{}}"#)]
fn compiler_artifact_parser_rejects_invalid_messages(#[case] message: &str) {
    assert_eq!(compiler_artifact_paths(message), None);
}

/// Reject malformed, irrelevant, and mismatched messages for a named library.
#[rstest]
#[case("not JSON")]
#[case(r#"{"reason":"build-script-executed"}"#)]
#[case(r#"{"reason":"compiler-artifact","filenames":[]}"#)]
#[case(r#"{"reason":"compiler-artifact","target":{},"filenames":[]}"#)]
#[case(r#"{"reason":"compiler-artifact","target":{"name":"fixture"}}"#)]
#[case(r#"{"reason":"compiler-artifact","target":{"name":"fixture"},"filenames":{}}"#)]
#[case(
    r#"{"reason":"compiler-artifact","target":{"name":"fixture"},"filenames":["/build/libfixture.a","/build/libfixture.txt"]}"#
)]
#[case(
    r#"{"reason":"compiler-artifact","target":{"name":"other"},"filenames":["/build/libother.rmeta"]}"#
)]
fn library_path_parser_rejects_unmatched_messages(#[case] message: &str) {
    assert_eq!(library_path_in_message(message, "fixture"), None);
}
