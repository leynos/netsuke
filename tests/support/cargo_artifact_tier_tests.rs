//! Regression cases for Cargo artefact tier preference and package scoping.

use super::{
    ArtefactFixture, ArtefactIdentity, LibraryArtefactSelection, library_path_in_messages,
};
use std::path::PathBuf;

/// Prefer one metadata-only check artefact over earlier codegen library files.
#[test]
fn stream_selector_prefers_metadata_only_tier_over_codegen_fallbacks() {
    let identity = ArtefactIdentity {
        package_id: "path+file:///workspace/fixture#fixture@0.1.0",
        source_path: "/workspace/fixture/src/lib.rs",
        manifest_path: Some("/workspace/fixture/Cargo.toml"),
    };
    let first_codegen = ArtefactFixture::library(
        identity,
        &[
            "/workspace/target/codegen-a/libfixture.rmeta",
            "/workspace/target/codegen-a/libfixture.rlib",
        ],
    )
    .render();
    let second_codegen = ArtefactFixture::library(
        identity,
        &[
            "/workspace/target/codegen-b/libfixture.rmeta",
            "/workspace/target/codegen-b/libfixture.rlib",
        ],
    )
    .render();
    let check =
        ArtefactFixture::library(identity, &["/workspace/target/check/libfixture.rmeta"]).render();

    assert_eq!(
        library_path_in_messages(
            &format!("{first_codegen}\n{second_codegen}\n{check}"),
            LibraryArtefactSelection {
                target_name: "fixture",
                expected_package_id: None,
            },
        )
        .expect("the unique metadata-only unit should win"),
        PathBuf::from("/workspace/target/check/libfixture.rmeta"),
    );
}

/// Reject two metadata-only paths even when a codegen fallback is also present.
#[test]
fn stream_selector_rejects_multiple_metadata_only_paths() {
    let identity = ArtefactIdentity {
        package_id: "path+file:///workspace/fixture#fixture@0.1.0",
        source_path: "/workspace/fixture/src/lib.rs",
        manifest_path: Some("/workspace/fixture/Cargo.toml"),
    };
    let first_check =
        ArtefactFixture::library(identity, &["/workspace/target/check-a/libfixture.rmeta"])
            .render();
    let second_check =
        ArtefactFixture::library(identity, &["/workspace/target/check-b/libfixture.rmeta"])
            .render();
    let codegen = ArtefactFixture::library(
        identity,
        &[
            "/workspace/target/codegen/libfixture.rmeta",
            "/workspace/target/codegen/libfixture.rlib",
        ],
    )
    .render();

    let error = library_path_in_messages(
        &format!("{first_check}\n{second_check}\n{codegen}"),
        LibraryArtefactSelection {
            target_name: "fixture",
            expected_package_id: None,
        },
    )
    .expect_err("distinct selected-tier paths must be ambiguous");
    assert!(error.to_string().contains("ambiguous library artefacts"));
}

/// Select a requested package exactly regardless of other same-name packages.
#[test]
fn package_scoped_selection_ignores_other_package_order() {
    let requested = ArtefactFixture::library(
        ArtefactIdentity {
            package_id: "opaque-package-id-requested",
            source_path: "/workspace/fixture/src/lib.rs",
            manifest_path: Some("/workspace/fixture/Cargo.toml"),
        },
        &["/workspace/target/requested/libfixture.rmeta"],
    )
    .render();
    let other = ArtefactFixture::library(
        ArtefactIdentity {
            package_id: "opaque-package-id-other",
            source_path: "/workspace/fixture/src/lib.rs",
            manifest_path: Some("/workspace/fixture/Cargo.toml"),
        },
        &["/workspace/target/other/libfixture.rmeta"],
    )
    .render();

    for messages in [
        format!("{requested}\n{other}"),
        format!("{other}\n{requested}"),
    ] {
        assert_eq!(
            library_path_in_messages(
                &messages,
                LibraryArtefactSelection {
                    target_name: "fixture",
                    expected_package_id: Some("opaque-package-id-requested"),
                },
            )
            .expect("the exact package ID should be selected"),
            PathBuf::from("/workspace/target/requested/libfixture.rmeta"),
        );
    }
}

/// Reject unscoped package ambiguity even when only one package is metadata-only.
#[test]
fn unscoped_selection_rejects_package_ambiguity_across_tiers() {
    let codegen = ArtefactFixture::library(
        ArtefactIdentity {
            package_id: "opaque-package-id-codegen",
            source_path: "/workspace/fixture/src/lib.rs",
            manifest_path: Some("/workspace/fixture/Cargo.toml"),
        },
        &[
            "/workspace/target/codegen/libfixture.rmeta",
            "/workspace/target/codegen/libfixture.rlib",
        ],
    )
    .render();
    let check = ArtefactFixture::library(
        ArtefactIdentity {
            package_id: "opaque-package-id-check",
            source_path: "/workspace/fixture/src/lib.rs",
            manifest_path: Some("/workspace/fixture/Cargo.toml"),
        },
        &["/workspace/target/check/libfixture.rmeta"],
    )
    .render();

    let error = library_path_in_messages(
        &format!("{codegen}\n{check}"),
        LibraryArtefactSelection {
            target_name: "fixture",
            expected_package_id: None,
        },
    )
    .expect_err("package identity ambiguity must be checked across tiers");
    assert!(error.to_string().contains("ambiguous library artefacts"));
}

/// Reject unscoped source ambiguity across metadata-only and codegen tiers.
#[test]
fn unscoped_selection_rejects_source_ambiguity_across_tiers() {
    let codegen = ArtefactFixture::library(
        ArtefactIdentity {
            package_id: "opaque-package-id",
            source_path: "/workspace/fixture/src/codegen.rs",
            manifest_path: Some("/workspace/fixture/Cargo.toml"),
        },
        &[
            "/workspace/target/codegen/libfixture.rmeta",
            "/workspace/target/codegen/libfixture.rlib",
        ],
    )
    .render();
    let check = ArtefactFixture::library(
        ArtefactIdentity {
            package_id: "opaque-package-id",
            source_path: "/workspace/fixture/src/check.rs",
            manifest_path: Some("/workspace/fixture/Cargo.toml"),
        },
        &["/workspace/target/check/libfixture.rmeta"],
    )
    .render();

    let error = library_path_in_messages(
        &format!("{codegen}\n{check}"),
        LibraryArtefactSelection {
            target_name: "fixture",
            expected_package_id: None,
        },
    )
    .expect_err("source identity ambiguity must be checked across tiers");
    assert!(error.to_string().contains("ambiguous library artefacts"));
}
