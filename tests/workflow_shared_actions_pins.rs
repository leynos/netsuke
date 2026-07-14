//! Verify shared-actions references are pinned to a full commit SHA.
//!
//! Dependabot owns the SHA value for each `leynos/shared-actions` action
//! reference and bumps callers one at a time, so this test does not assert
//! that every reference shares an identical pin. It only asserts the shape
//! of each reference: the correct `.github/actions/<name>` path, pinned to a
//! 40-character lowercase-hex commit SHA rather than a mutable branch or tag
//! such as `main`.

use anyhow::{Context, Result, ensure};
use std::fs;
use std::path::{Path, PathBuf};

const SHARED_ACTIONS_MARKER: &str = "leynos/shared-actions/.github/actions/";

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

fn extract_shared_actions_uses(contents: &str) -> Vec<String> {
    contents
        .lines()
        .filter_map(|line| {
            let start = line.find(SHARED_ACTIONS_MARKER)?;
            let rest = line.get(start..)?;
            let token = rest.split_whitespace().next()?;
            (!token.is_empty()).then(|| token.to_owned())
        })
        .collect()
}

/// Returns true when `reference` is `leynos/shared-actions/.github/actions/<name>`
/// pinned to a full 40-character lowercase-hex commit SHA.
fn is_pinned_shared_action_ref(reference: &str) -> bool {
    let Some(rest) = reference.strip_prefix(SHARED_ACTIONS_MARKER) else {
        return false;
    };
    let Some((name, pin)) = rest.split_once('@') else {
        return false;
    };
    !name.is_empty()
        && pin.len() == 40
        && pin
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
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
