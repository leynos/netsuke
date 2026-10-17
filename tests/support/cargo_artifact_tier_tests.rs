//! Regression cases for Cargo artefact tier preference and package scoping.

use super::{artefact_message, library_path_in_messages, library_path_in_messages_for_package};
use std::path::PathBuf;

/// Prefer one metadata-only check artefact over earlier codegen library files.
#[test]
fn stream_selector_prefers_metadata_only_tier_over_codegen_fallbacks() {
    let package_id = "path+file:///workspace/fixture#fixture@0.1.0";
    let source_path = "/workspace/fixture/src/lib.rs";
    let manifest_path = Some("/workspace/fixture/Cargo.toml");
    let first_codegen = artefact_message(
        package_id,
        source_path,
        manifest_path,
        &[
            "/workspace/target/codegen-a/libfixture.rmeta",
            "/workspace/target/codegen-a/libfixture.rlib",
        ],
    );
    let second_codegen = artefact_message(
        package_id,
        source_path,
        manifest_path,
        &[
            "/workspace/target/codegen-b/libfixture.rmeta",
            "/workspace/target/codegen-b/libfixture.rlib",
        ],
    );
    let check = artefact_message(
        package_id,
        source_path,
        manifest_path,
        &["/workspace/target/check/libfixture.rmeta"],
    );

    assert_eq!(
        library_path_in_messages(
            &format!("{first_codegen}\n{second_codegen}\n{check}"),
            "fixture",
        )
        .expect("the unique metadata-only unit should win"),
        PathBuf::from("/workspace/target/check/libfixture.rmeta"),
    );
}

/// Reject two metadata-only paths even when a codegen fallback is also present.
#[test]
fn stream_selector_rejects_multiple_metadata_only_paths() {
    let package_id = "path+file:///workspace/fixture#fixture@0.1.0";
    let source_path = "/workspace/fixture/src/lib.rs";
    let manifest_path = Some("/workspace/fixture/Cargo.toml");
    let first_check = artefact_message(
        package_id,
        source_path,
        manifest_path,
        &["/workspace/target/check-a/libfixture.rmeta"],
    );
    let second_check = artefact_message(
        package_id,
        source_path,
        manifest_path,
        &["/workspace/target/check-b/libfixture.rmeta"],
    );
    let codegen = artefact_message(
        package_id,
        source_path,
        manifest_path,
        &[
            "/workspace/target/codegen/libfixture.rmeta",
            "/workspace/target/codegen/libfixture.rlib",
        ],
    );

    let error = library_path_in_messages(
        &format!("{first_check}\n{second_check}\n{codegen}"),
        "fixture",
    )
    .expect_err("distinct selected-tier paths must be ambiguous");
    assert!(error.to_string().contains("ambiguous library artefacts"));
}

/// Select a requested package exactly regardless of other same-name packages.
#[test]
fn package_scoped_selection_ignores_other_package_order() {
    let requested = artefact_message(
        "opaque-package-id-requested",
        "/workspace/fixture/src/lib.rs",
        Some("/workspace/fixture/Cargo.toml"),
        &["/workspace/target/requested/libfixture.rmeta"],
    );
    let other = artefact_message(
        "opaque-package-id-other",
        "/workspace/fixture/src/lib.rs",
        Some("/workspace/fixture/Cargo.toml"),
        &["/workspace/target/other/libfixture.rmeta"],
    );

    for messages in [
        format!("{requested}\n{other}"),
        format!("{other}\n{requested}"),
    ] {
        assert_eq!(
            library_path_in_messages_for_package(
                &messages,
                "fixture",
                "opaque-package-id-requested",
            )
            .expect("the exact package ID should be selected"),
            PathBuf::from("/workspace/target/requested/libfixture.rmeta"),
        );
    }
}

/// Reject unscoped package ambiguity even when only one package is metadata-only.
#[test]
fn unscoped_selection_rejects_package_ambiguity_across_tiers() {
    let codegen = artefact_message(
        "opaque-package-id-codegen",
        "/workspace/fixture/src/lib.rs",
        Some("/workspace/fixture/Cargo.toml"),
        &[
            "/workspace/target/codegen/libfixture.rmeta",
            "/workspace/target/codegen/libfixture.rlib",
        ],
    );
    let check = artefact_message(
        "opaque-package-id-check",
        "/workspace/fixture/src/lib.rs",
        Some("/workspace/fixture/Cargo.toml"),
        &["/workspace/target/check/libfixture.rmeta"],
    );

    let error = library_path_in_messages(&format!("{codegen}\n{check}"), "fixture")
        .expect_err("package identity ambiguity must be checked across tiers");
    assert!(error.to_string().contains("ambiguous library artefacts"));
}

/// Reject unscoped source ambiguity across metadata-only and codegen tiers.
#[test]
fn unscoped_selection_rejects_source_ambiguity_across_tiers() {
    let codegen = artefact_message(
        "opaque-package-id",
        "/workspace/fixture/src/codegen.rs",
        Some("/workspace/fixture/Cargo.toml"),
        &[
            "/workspace/target/codegen/libfixture.rmeta",
            "/workspace/target/codegen/libfixture.rlib",
        ],
    );
    let check = artefact_message(
        "opaque-package-id",
        "/workspace/fixture/src/check.rs",
        Some("/workspace/fixture/Cargo.toml"),
        &["/workspace/target/check/libfixture.rmeta"],
    );

    let error = library_path_in_messages(&format!("{codegen}\n{check}"), "fixture")
        .expect_err("source identity ambiguity must be checked across tiers");
    assert!(error.to_string().contains("ambiguous library artefacts"));
}
