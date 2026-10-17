//! Regression tests for stream-level Cargo artefact selection.

use super::{library_path_in_messages, library_path_in_messages_for_package};
use rstest::rstest;
use std::path::PathBuf;

/// Describe one Cargo artefact fixture without repeating its default library profile.
struct ArtefactFixture<'a> {
    package_id: &'a str,
    source_path: &'a str,
    manifest_path: Option<&'a str>,
    kind: &'a [&'a str],
    crate_types: &'a [&'a str],
    is_test: bool,
    filenames: &'a [&'a str],
}

impl<'a> ArtefactFixture<'a> {
    /// Create a normal library fixture with an explicit source and file list.
    fn library(
        package_id: &'a str,
        source_path: &'a str,
        manifest_path: Option<&'a str>,
        filenames: &'a [&'a str],
    ) -> Self {
        Self {
            package_id,
            source_path,
            manifest_path,
            kind: &["lib"],
            crate_types: &["lib"],
            is_test: false,
            filenames,
        }
    }

    /// Set the target classification for non-library control messages.
    fn with_target(self, kind: &'a [&'a str], crate_types: &'a [&'a str]) -> Self {
        Self {
            kind,
            crate_types,
            ..self
        }
    }

    /// Mark the fixture as a test-profile compilation unit.
    fn with_test_profile(self) -> Self {
        Self {
            is_test: true,
            ..self
        }
    }

    /// Render the fixture as one Cargo `compiler-artifact` JSON message.
    fn render(self) -> String {
        let target = serde_json::json!({
            "name": "fixture",
            "src_path": self.source_path,
            "kind": self.kind,
            "crate_types": self.crate_types,
        });
        let mut message = serde_json::Map::from_iter([
            (
                String::from("reason"),
                serde_json::json!("compiler-artifact"),
            ),
            (
                String::from("package_id"),
                serde_json::json!(self.package_id),
            ),
            (String::from("target"), target),
            (
                String::from("profile"),
                serde_json::json!({"test": self.is_test}),
            ),
            (String::from("filenames"), serde_json::json!(self.filenames)),
        ]);
        if let Some(path) = self.manifest_path {
            message.insert(String::from("manifest_path"), serde_json::json!(path));
        }
        serde_json::Value::Object(message).to_string()
    }
}

/// Render the default non-test library fixture used by tier-selection cases.
fn artefact_message(
    package_id: &str,
    source_path: &str,
    manifest_path: Option<&str>,
    filenames: &[&str],
) -> String {
    ArtefactFixture::library(package_id, source_path, manifest_path, filenames).render()
}

/// Ignore later test-harness metadata regardless of Cargo message order.
#[test]
fn stream_selector_ignores_test_harness_for_same_target_name() {
    let library = ArtefactFixture::library(
        "path+file:///workspace/fixture#fixture@0.1.0",
        "/workspace/fixture/src/lib.rs",
        Some("/workspace/fixture/Cargo.toml"),
        &["/workspace/target/libfixture.rmeta"],
    )
    .render();
    let test_harness = ArtefactFixture::library(
        "path+file:///workspace/fixture#fixture@0.1.0",
        "/workspace/fixture/src/lib.rs",
        Some("/workspace/fixture/Cargo.toml"),
        &["/workspace/target/libfixture-test.rmeta"],
    )
    .with_test_profile()
    .render();

    for messages in [
        format!("{library}\n{test_harness}"),
        format!("{test_harness}\n{library}"),
    ] {
        assert_eq!(
            library_path_in_messages(&messages, "fixture")
                .expect("normal library artefact should be selected"),
            PathBuf::from("/workspace/target/libfixture.rmeta"),
        );
    }
}

/// Reject binary and procedural-macro targets even when their names match.
#[rstest]
#[case(&["bin"], &["bin"])]
#[case(&["proc-macro"], &["proc-macro"])]
fn stream_selector_rejects_non_library_targets(
    #[case] kind: &[&str],
    #[case] crate_types: &[&str],
) {
    let message = ArtefactFixture::library(
        "path+file:///workspace/fixture#fixture@0.1.0",
        "/workspace/fixture/src/main.rs",
        Some("/workspace/fixture/Cargo.toml"),
        &["/workspace/target/fixture.rmeta"],
    )
    .with_target(kind, crate_types)
    .render();

    assert!(library_path_in_messages(&message, "fixture").is_err());
}

/// Reject multiple matching host and target artefacts instead of choosing the last.
#[test]
fn stream_selector_rejects_distinct_artefact_paths_for_one_package() {
    let host = ArtefactFixture::library(
        "path+file:///workspace/fixture#fixture@0.1.0",
        "/workspace/fixture/src/lib.rs",
        Some("/workspace/fixture/Cargo.toml"),
        &["/workspace/target/host/libfixture.rmeta"],
    )
    .render();
    let target = ArtefactFixture::library(
        "path+file:///workspace/fixture#fixture@0.1.0",
        "/workspace/fixture/src/lib.rs",
        Some("/workspace/fixture/Cargo.toml"),
        &["/workspace/target/target/libfixture.rmeta"],
    )
    .render();

    let error = library_path_in_messages(&format!("{host}\n{target}"), "fixture")
        .expect_err("distinct host and target artefact paths must be ambiguous");
    assert!(error.to_string().contains("ambiguous library artefacts"));
}

/// Reject matching artefacts from distinct packages even when paths coincide.
#[test]
fn stream_selector_rejects_distinct_package_ids() {
    let first = ArtefactFixture::library(
        "path+file:///workspace/first#fixture@0.1.0",
        "/workspace/fixture/src/lib.rs",
        Some("/workspace/fixture/Cargo.toml"),
        &["/workspace/target/libfixture.rmeta"],
    )
    .render();
    let second = ArtefactFixture::library(
        "path+file:///workspace/second#fixture@0.1.0",
        "/workspace/fixture/src/lib.rs",
        Some("/workspace/fixture/Cargo.toml"),
        &["/workspace/target/libfixture.rmeta"],
    )
    .render();

    let error = library_path_in_messages(&format!("{first}\n{second}"), "fixture")
        .expect_err("different packages must remain distinct compilation units");
    assert!(error.to_string().contains("ambiguous library artefacts"));
}

/// Accept repeated messages when they describe the exact same unit and artefact.
#[test]
fn stream_selector_accepts_identical_duplicate_messages() {
    let message = ArtefactFixture::library(
        "path+file:///workspace/fixture#fixture@0.1.0",
        "/workspace/fixture/src/lib.rs",
        None,
        &["/workspace/target/libfixture.rmeta"],
    )
    .render();

    assert_eq!(
        library_path_in_messages(&format!("{message}\n{message}"), "fixture")
            .expect("identical compilation units should be accepted"),
        PathBuf::from("/workspace/target/libfixture.rmeta"),
    );
}

/// Report missing target and malformed compilation-unit identity fields.
#[rstest]
#[case(r#"{"reason":"compiler-artifact"}"#, "target object")]
#[case(
    r#"{"reason":"compiler-artifact","package_id":"fixture","manifest_path":null,"target":{"name":"fixture","src_path":"/fixture/src/lib.rs","kind":["lib"],"crate_types":["lib"]},"profile":{"test":false},"filenames":["/target/libfixture.rmeta"]}"#,
    "manifest_path"
)]
#[case(
    r#"{"reason":"compiler-artifact","target":{"name":"fixture","src_path":"/fixture/src/lib.rs","kind":["lib"],"crate_types":["lib"]},"profile":{"test":false},"filenames":["/target/libfixture.rmeta"]}"#,
    "package_id"
)]
#[case(
    r#"{"reason":"compiler-artifact","package_id":"fixture","target":{"name":"fixture","kind":["lib"],"crate_types":["lib"]},"profile":{"test":false},"filenames":["/target/libfixture.rmeta"]}"#,
    "target.src_path"
)]
#[case(
    r#"{"reason":"compiler-artifact","package_id":"fixture","target":{"name":"fixture","src_path":"/fixture/src/lib.rs","kind":["lib"],"crate_types":["lib"]},"filenames":["/target/libfixture.rmeta"]}"#,
    "profile.test"
)]
fn stream_selector_reports_missing_or_malformed_identity(
    #[case] message: &str,
    #[case] expected_field: &str,
) {
    let error = library_path_in_messages(message, "fixture")
        .expect_err("incomplete compilation-unit identity must fail");

    assert!(
        error.to_string().contains(expected_field),
        "unexpected error for {expected_field}: {error}"
    );
}

/// Prefer metadata, then fall back to the library path within one unit.
#[rstest]
#[case(&["/target/libfixture.rlib", "/target/libfixture.rmeta"], "/target/libfixture.rmeta")]
#[case(&["/target/libfixture.rlib"], "/target/libfixture.rlib")]
fn stream_selector_prefers_metadata_with_library_fallback(
    #[case] filenames: &[&str],
    #[case] expected: &str,
) {
    let message = ArtefactFixture::library(
        "path+file:///workspace/fixture#fixture@0.1.0",
        "/workspace/fixture/src/lib.rs",
        None,
        filenames,
    )
    .render();

    assert_eq!(
        library_path_in_messages(&message, "fixture")
            .expect("one normal library artefact should be selected"),
        PathBuf::from(expected),
    );
}

#[path = "cargo_artifact_tier_tests.rs"]
mod tier_tests;
