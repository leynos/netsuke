//! Regression tests for stream-level Cargo artefact selection.

use super::{LibraryArtefactSelection, library_path_in_messages};
use rstest::rstest;
use std::path::PathBuf;

/// Identify one fixture package and the source file Cargo reports for it.
#[derive(Clone, Copy)]
struct ArtefactIdentity<'a> {
    package_id: &'a str,
    source_path: &'a str,
    manifest_path: Option<&'a str>,
}

/// Describe one Cargo artefact fixture without repeating its default library profile.
struct ArtefactFixture<'a> {
    identity: ArtefactIdentity<'a>,
    kind: &'a [&'a str],
    crate_types: &'a [&'a str],
    is_test: bool,
    filenames: &'a [&'a str],
}

impl<'a> ArtefactFixture<'a> {
    /// Create a normal library fixture with an explicit source and file list.
    fn library(identity: ArtefactIdentity<'a>, filenames: &'a [&'a str]) -> Self {
        Self {
            identity,
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
    fn render(&self) -> String {
        let target = serde_json::json!({
            "name": "fixture",
            "src_path": self.identity.source_path,
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
                serde_json::json!(self.identity.package_id),
            ),
            (String::from("target"), target),
            (
                String::from("profile"),
                serde_json::json!({"test": self.is_test}),
            ),
            (String::from("filenames"), serde_json::json!(self.filenames)),
        ]);
        if let Some(path) = self.identity.manifest_path {
            message.insert(String::from("manifest_path"), serde_json::json!(path));
        }
        serde_json::Value::Object(message).to_string()
    }
}

/// Ignore later test-harness metadata regardless of Cargo message order.
#[test]
fn stream_selector_ignores_test_harness_for_same_target_name() {
    let library = ArtefactFixture::library(
        ArtefactIdentity {
            package_id: "path+file:///workspace/fixture#fixture@0.1.0",
            source_path: "/workspace/fixture/src/lib.rs",
            manifest_path: Some("/workspace/fixture/Cargo.toml"),
        },
        &["/workspace/target/libfixture.rmeta"],
    )
    .render();
    let test_harness = ArtefactFixture::library(
        ArtefactIdentity {
            package_id: "path+file:///workspace/fixture#fixture@0.1.0",
            source_path: "/workspace/fixture/src/lib.rs",
            manifest_path: Some("/workspace/fixture/Cargo.toml"),
        },
        &["/workspace/target/libfixture-test.rmeta"],
    )
    .with_test_profile()
    .render();

    for messages in [
        format!("{library}\n{test_harness}"),
        format!("{test_harness}\n{library}"),
    ] {
        assert_eq!(
            library_path_in_messages(
                &messages,
                LibraryArtefactSelection {
                    target_name: "fixture",
                    expected_package_id: None,
                },
            )
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
        ArtefactIdentity {
            package_id: "path+file:///workspace/fixture#fixture@0.1.0",
            source_path: "/workspace/fixture/src/main.rs",
            manifest_path: Some("/workspace/fixture/Cargo.toml"),
        },
        &["/workspace/target/fixture.rmeta"],
    )
    .with_target(kind, crate_types)
    .render();

    assert!(
        library_path_in_messages(
            &message,
            LibraryArtefactSelection {
                target_name: "fixture",
                expected_package_id: None,
            },
        )
        .is_err()
    );
}

/// Vary one source of unit ambiguity while keeping the rejection contract fixed.
#[derive(Clone, Copy)]
enum UnitAmbiguity {
    /// Use one package with distinct host and target artefact paths.
    ArtefactPath,
    /// Use distinct package IDs with the same artefact path.
    PackageId,
}

/// Reject distinct target units regardless of whether the path or package differs.
#[rstest]
#[case::distinct_artefact_paths(UnitAmbiguity::ArtefactPath)]
#[case::distinct_package_ids(UnitAmbiguity::PackageId)]
fn stream_selector_rejects_distinct_units(#[case] ambiguity: UnitAmbiguity) {
    let first_identity = ArtefactIdentity {
        package_id: "path+file:///workspace/fixture#fixture@0.1.0",
        source_path: "/workspace/fixture/src/lib.rs",
        manifest_path: Some("/workspace/fixture/Cargo.toml"),
    };
    let (second_identity, first_path, second_path) = match ambiguity {
        UnitAmbiguity::ArtefactPath => (
            first_identity,
            "/workspace/target/host/libfixture.rmeta",
            "/workspace/target/target/libfixture.rmeta",
        ),
        UnitAmbiguity::PackageId => (
            ArtefactIdentity {
                package_id: "path+file:///workspace/other#fixture@0.1.0",
                ..first_identity
            },
            "/workspace/target/libfixture.rmeta",
            "/workspace/target/libfixture.rmeta",
        ),
    };
    let first = ArtefactFixture::library(first_identity, &[first_path]).render();
    let second = ArtefactFixture::library(second_identity, &[second_path]).render();
    let error = library_path_in_messages(
        &format!("{first}\n{second}"),
        LibraryArtefactSelection {
            target_name: "fixture",
            expected_package_id: None,
        },
    )
    .expect_err("distinct matching target units must be ambiguous");

    assert!(error.to_string().contains("ambiguous library artefacts"));
}

/// Accept repeated messages when they describe the exact same unit and artefact.
#[test]
fn stream_selector_accepts_identical_duplicate_messages() {
    let message = ArtefactFixture::library(
        ArtefactIdentity {
            package_id: "path+file:///workspace/fixture#fixture@0.1.0",
            source_path: "/workspace/fixture/src/lib.rs",
            manifest_path: None,
        },
        &["/workspace/target/libfixture.rmeta"],
    )
    .render();

    assert_eq!(
        library_path_in_messages(
            &format!("{message}\n{message}"),
            LibraryArtefactSelection {
                target_name: "fixture",
                expected_package_id: None,
            },
        )
        .expect("identical compilation units should be accepted"),
        PathBuf::from("/workspace/target/libfixture.rmeta"),
    );
}

/// Pair a malformed Cargo message with its expected diagnostic field.
#[derive(Clone, Copy)]
struct InvalidIdentityMessage {
    text: &'static str,
    expected_field: &'static str,
}

/// Report missing target and malformed compilation-unit identity fields.
#[rstest]
#[case(
    InvalidIdentityMessage {
        text: r#"{"reason":"compiler-artifact"}"#,
        expected_field: "target object",
    }
)]
#[case(
    InvalidIdentityMessage {
        text: r#"{"reason":"compiler-artifact","package_id":"fixture","manifest_path":null,"target":{"name":"fixture","src_path":"/fixture/src/lib.rs","kind":["lib"],"crate_types":["lib"]},"profile":{"test":false},"filenames":["/target/libfixture.rmeta"]}"#,
        expected_field: "manifest_path",
    }
)]
#[case(
    InvalidIdentityMessage {
        text: r#"{"reason":"compiler-artifact","target":{"name":"fixture","src_path":"/fixture/src/lib.rs","kind":["lib"],"crate_types":["lib"]},"profile":{"test":false},"filenames":["/target/libfixture.rmeta"]}"#,
        expected_field: "package_id",
    }
)]
#[case(
    InvalidIdentityMessage {
        text: r#"{"reason":"compiler-artifact","package_id":"fixture","target":{"name":"fixture","kind":["lib"],"crate_types":["lib"]},"profile":{"test":false},"filenames":["/target/libfixture.rmeta"]}"#,
        expected_field: "target.src_path",
    }
)]
#[case(
    InvalidIdentityMessage {
        text: r#"{"reason":"compiler-artifact","package_id":"fixture","target":{"name":"fixture","src_path":"/fixture/src/lib.rs","kind":["lib"],"crate_types":["lib"]},"filenames":["/target/libfixture.rmeta"]}"#,
        expected_field: "profile.test",
    }
)]
fn stream_selector_reports_missing_or_malformed_identity(#[case] invalid: InvalidIdentityMessage) {
    let error = library_path_in_messages(
        invalid.text,
        LibraryArtefactSelection {
            target_name: "fixture",
            expected_package_id: None,
        },
    )
    .expect_err("incomplete compilation-unit identity must fail");

    assert!(
        error.to_string().contains(invalid.expected_field),
        "unexpected error for {}: {error}",
        invalid.expected_field,
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
        ArtefactIdentity {
            package_id: "path+file:///workspace/fixture#fixture@0.1.0",
            source_path: "/workspace/fixture/src/lib.rs",
            manifest_path: None,
        },
        filenames,
    )
    .render();

    assert_eq!(
        library_path_in_messages(
            &message,
            LibraryArtefactSelection {
                target_name: "fixture",
                expected_package_id: None,
            },
        )
        .expect("one normal library artefact should be selected"),
        PathBuf::from(expected),
    );
}

#[path = "cargo_artifact_tier_tests.rs"]
mod tier_tests;
