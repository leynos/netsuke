//! Verify shared-actions pinning remains consistent across workflows.

use anyhow::{Context, Result, ensure};
use std::collections::BTreeSet;
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

fn extract_shared_actions_pins(contents: &str) -> Vec<String> {
    contents
        .lines()
        .filter_map(|line| {
            let marker = "leynos/shared-actions/.github/actions/";
            if !line.contains(marker) {
                return None;
            }
            let pin = line
                .split('@')
                .nth(1)
                .map(str::trim)
                .and_then(|segment| segment.split_whitespace().next())
                .filter(|value| !value.is_empty())?;
            Some(pin.to_owned())
        })
        .collect()
}

#[test]
fn unit_extracts_pins_from_workflow_lines() {
    let sample = r"
      - uses: leynos/shared-actions/.github/actions/setup-rust@abc123
      - uses: leynos/shared-actions/.github/actions/generate-coverage@abc123
    ";

    let pins = extract_shared_actions_pins(sample);

    assert_eq!(pins, vec!["abc123", "abc123"]);
}

#[test]
fn behavioural_shared_actions_pins_are_consistent() -> Result<()> {
    let workflows = fs::read_dir(workflow_dir()).context("workflow directory should exist")?;
    let mut pins = BTreeSet::new();

    for entry in workflows {
        let workflow_entry = entry.context("workflow directory entries should be readable")?;
        let path = workflow_entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("yml") {
            continue;
        }
        for pin in extract_shared_actions_pins(&read_workflow(&path)?) {
            pins.insert(pin);
        }
    }

    ensure!(
        !pins.is_empty(),
        "expected at least one shared-actions pin in workflows"
    );
    ensure!(
        pins.len() == 1,
        "shared-actions pins should be identical across workflows, found {pins:?}"
    );
    Ok(())
}
