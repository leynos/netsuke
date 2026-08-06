//! Verify shared-actions references are pinned to a full commit SHA.
//!
//! Dependabot owns the SHA value for each `leynos/shared-actions` action
//! reference and bumps callers one at a time, so this sweep does not assert
//! that every reference shares an identical pin. It only asserts the shape
//! of each reference: the correct `.github/actions/<name>` path, pinned to a
//! 40-character lowercase-hex commit SHA rather than a mutable branch or tag
//! such as `main`. The stricter agreement check — deriving one pin the
//! toolchain-contract workflows must share — lives in
//! `polonius_toolchain_contract`; both consume the parsing helpers in
//! `tests/support/shared_actions.rs`.

#[path = "support/shared_actions.rs"]
pub mod shared_actions;

use anyhow::{Context, Result, ensure};
use shared_actions::{consistent_pin, extract_shared_actions_uses, split_shared_action_ref};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workflow_dir() -> PathBuf {
    repo_root().join(".github").join("workflows")
}

fn read_workflow(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .with_context(|| format!("workflow file {} should be readable", path.display()))
}

/// Returns true when `reference` is `leynos/shared-actions/.github/actions/<name>`
/// pinned to a full 40-character lowercase-hex commit SHA.
fn is_pinned_shared_action_ref(reference: &str) -> bool {
    split_shared_action_ref(reference)
        .is_some_and(|(name, pin)| !name.is_empty() && shared_actions::is_commit_sha_pin(pin))
}

#[test]
fn unit_extracts_uses_from_workflow_lines() {
    let sample = r"
      - uses: leynos/shared-actions/.github/actions/setup-rust@0123456789abcdef0123456789abcdef01234567
      - uses: leynos/shared-actions/.github/actions/generate-coverage@0123456789abcdef0123456789abcdef01234567
    ";

    let uses = extract_shared_actions_uses(sample);

    assert_eq!(
        uses,
        vec![
            "leynos/shared-actions/.github/actions/setup-rust@0123456789abcdef0123456789abcdef01234567",
            "leynos/shared-actions/.github/actions/generate-coverage@0123456789abcdef0123456789abcdef01234567",
        ]
    );
}

#[test]
fn unit_rejects_refs_not_pinned_to_a_commit_sha() {
    assert!(is_pinned_shared_action_ref(
        "leynos/shared-actions/.github/actions/setup-rust@0123456789abcdef0123456789abcdef01234567"
    ));
    assert!(!is_pinned_shared_action_ref(
        "leynos/shared-actions/.github/actions/setup-rust@main"
    ));
    assert!(!is_pinned_shared_action_ref(
        "leynos/shared-actions/.github/actions/setup-rust@0123456789ABCDEF0123456789ABCDEF01234567"
    ));
    assert!(!is_pinned_shared_action_ref(
        "leynos/shared-actions/.github/workflows/mutation-cargo.yml@0123456789abcdef0123456789abcdef01234567"
    ));
}

#[test]
fn behavioural_shared_actions_pins_are_full_commit_shas() -> Result<()> {
    let workflows = fs::read_dir(workflow_dir()).context("workflow directory should exist")?;
    let mut refs = Vec::new();

    for entry in workflows {
        let workflow_entry = entry.context("workflow directory entries should be readable")?;
        let path = workflow_entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("yml") {
            continue;
        }
        refs.extend(extract_shared_actions_uses(&read_workflow(&path)?));
    }

    ensure!(
        !refs.is_empty(),
        "expected at least one shared-actions action reference in workflows"
    );
    for reference in &refs {
        ensure!(
            is_pinned_shared_action_ref(reference),
            "shared-actions action reference should be pinned to a 40-hex commit SHA, found {reference:?}"
        );
    }
    Ok(())
}

const VALID_REF_A: &str =
    "leynos/shared-actions/.github/actions/setup-rust@0123456789abcdef0123456789abcdef01234567";

/// Build a full reference for `pin` on an arbitrary action name.
fn reference_with_pin(pin: &str) -> String {
    format!("leynos/shared-actions/.github/actions/rust-build-release@{pin}")
}

#[test]
fn unit_consistent_pin_accepts_agreeing_references() -> Result<()> {
    let refs = vec![
        VALID_REF_A.to_owned(),
        reference_with_pin("0123456789abcdef0123456789abcdef01234567"),
    ];
    let pin = consistent_pin(&refs)?;
    ensure!(
        pin == "0123456789abcdef0123456789abcdef01234567",
        "agreeing references should yield their shared pin, got {pin:?}"
    );
    Ok(())
}

#[test]
fn unit_consistent_pin_rejects_an_empty_reference_list() {
    let error = consistent_pin(&[]).expect_err("no references should reject");
    assert!(
        error.to_string().contains("at least one"),
        "the empty-list rejection should say so; got {error:?}"
    );
}

#[test]
fn unit_consistent_pin_rejects_malformed_references() {
    for reference in [
        "leynos/shared-actions/.github/actions/setup-rust", // no pin separator
        "actions/checkout@0123456789abcdef0123456789abcdef01234567", // wrong prefix
    ] {
        let error = consistent_pin(&[reference.to_owned()])
            .expect_err("a malformed reference should reject");
        assert!(
            error.to_string().contains("malformed"),
            "{reference:?} should be reported as malformed; got {error:?}"
        );
    }
}

#[test]
fn unit_consistent_pin_rejects_non_sha_pins() {
    for pin in [
        "main",
        "v1.2.3",
        "0123456789ABCDEF0123456789ABCDEF01234567",
        "abc123",
    ] {
        let error =
            consistent_pin(&[reference_with_pin(pin)]).expect_err("a non-SHA pin should reject");
        assert!(
            error.to_string().contains("40-hex commit SHA"),
            "pin {pin:?} should be rejected for its shape; got {error:?}"
        );
    }
}

#[test]
fn unit_consistent_pin_rejects_disagreeing_shas() {
    let refs = vec![
        VALID_REF_A.to_owned(),
        reference_with_pin("fedcba9876543210fedcba9876543210fedcba98"),
    ];
    let error = consistent_pin(&refs).expect_err("disagreeing pins should reject");
    assert!(
        error.to_string().contains("disagree"),
        "the disagreement rejection should say so; got {error:?}"
    );
}
