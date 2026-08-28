//! Validate release workflow wiring for shared actions.

mod common;

use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use common::workflow_contents;
use rstest::rstest;
use serde_yaml::{Mapping, Value as YamlValue};
use test_support::fs as test_fs;

fn mapping_value<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a YamlValue> {
    mapping.get(YamlValue::String(key.to_owned()))
}

fn release_admission_command(workflow: &YamlValue) -> Result<&str> {
    let jobs = workflow
        .as_mapping()
        .and_then(|root| mapping_value(root, "jobs"))
        .and_then(YamlValue::as_mapping)
        .context("release workflow should define jobs")?;
    let admission = mapping_value(jobs, "release-admission-canaries")
        .and_then(YamlValue::as_mapping)
        .context("release workflow should define the canary admission job")?;
    let steps = mapping_value(admission, "steps")
        .and_then(YamlValue::as_sequence)
        .context("canary admission should define steps")?;
    let admission_step = steps
        .iter()
        .filter_map(YamlValue::as_mapping)
        .find(|step| {
            mapping_value(step, "name").and_then(YamlValue::as_str)
                == Some("Require successful pinned downstream canaries")
        })
        .context("canary admission should query downstream runs")?;

    mapping_value(admission_step, "run")
        .and_then(YamlValue::as_str)
        .context("canary admission query should be a shell script")
}

fn release_admission_script() -> Result<String> {
    test_fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".github/scripts/require-release-admission-canaries.sh"),
    )
    .context("read release-admission canary script")
}

/// Require the release workflow to invoke the canary admission boundary.
fn require_release_admission_workflow_wiring(
    contents: &str,
    admission_command: &str,
) -> Result<()> {
    ensure!(
        contents.contains("release-admission-canaries:"),
        "release workflow should define the downstream canary admission job"
    );
    ensure!(
        contents.contains("- release-admission-canaries"),
        "release publication should require successful downstream canaries"
    );
    ensure!(
        admission_command == "bash .github/scripts/require-release-admission-canaries.sh",
        "release workflow should execute the tested canary-admission script"
    );

    Ok(())
}

/// Require every release-admission canary to retain its downstream revision.
fn require_pinned_canary_revisions(admission_script: &str) -> Result<()> {
    ensure!(
        admission_script
            .contains("leynos/repovec-appliance 6be365b4b30ef48537add5719a9b387ccc41777f")
            && admission_script.contains("leynos/mxd 8146278cc82506c222bb78d4f3fc05c12ed95b41")
            && admission_script
                .contains("leynos/ortho-config b42b5d0adfacd79456d2a2f9edbf9f561aac943b"),
        "release workflow should keep every v0.1.0 canary revision pinned"
    );

    Ok(())
}

/// Require the admission query to select runs for the exact downstream revision.
fn require_exact_revision_run_lookup(admission_script: &str) -> Result<()> {
    ensure!(
        admission_script.contains("head_sha=${revision}&per_page=100"),
        "canary admission should page the exact downstream revision's runs"
    );

    Ok(())
}

/// Require canary evidence to come from the trusted successful workflow run.
fn require_successful_trusted_run_evidence(admission_script: &str) -> Result<()> {
    ensure!(
        admission_script.contains(".name == \\\"${workflow_name}\\\"")
            && admission_script.contains(".conclusion == \\\"success\\\""),
        "canary admission should require the named workflow to succeed"
    );
    ensure!(
        admission_script.contains("actions/workflows/${workflow_id}/runs")
            && admission_script.contains(".workflow_id == ${workflow_id}")
            && admission_script.contains(".path == \\\".github/workflows/netsuke-canary.yml\\\"")
            && admission_script.contains(".event == \\\"push\\\"")
            && admission_script.contains(".head_branch == \\\"${branch}\\\"")
            && admission_script.contains(".head_sha == \\\"${revision}\\\"")
            && admission_script.contains("candidate ${GITHUB_SHA}"),
        "canary admission should bind a trusted workflow run to the published revision"
    );

    Ok(())
}

#[test]
fn behavioural_release_workflow_uses_shared_actions() {
    let contents = workflow_contents("release.yml").expect("release workflow should be readable");

    assert!(
        contents.contains("determine-release-modes@"),
        "release workflow should use shared determine-release-modes action"
    );
    assert!(
        contents.contains("ensure-cargo-version@"),
        "release workflow should use shared ensure-cargo-version action"
    );
    assert!(
        contents.contains("export-cargo-metadata@"),
        "release workflow should use shared export-cargo-metadata action"
    );
    assert!(
        contents.contains("upload-release-assets@"),
        "release workflow should use shared upload-release-assets action"
    );
}

#[test]
fn behavioural_release_workflow_exports_bin_name() {
    let contents = workflow_contents("release.yml").expect("release workflow should be readable");

    assert!(
        contents.contains("fields: bin-name"),
        "release workflow should export the bin-name field"
    );
    assert!(
        contents.contains("bin-name: ${{ needs.metadata.outputs.bin_name }}"),
        "release workflow should pass bin-name to upload-release-assets"
    );
}

#[test]
fn behavioural_release_workflow_wires_release_modes_outputs() {
    let contents = workflow_contents("release.yml").expect("release workflow should be readable");

    assert!(
        contents
            .contains("check-tag: ${{ fromJSON(steps.release_modes.outputs['should-publish']) }}"),
        "release workflow should gate tag checking on should-publish output"
    );
    assert!(
        contents.contains("should_publish: ${{ steps.release_modes.outputs['should-publish'] }}"),
        "release workflow should capture should-publish output"
    );
    assert!(
        contents.contains("dry_run: ${{ steps.release_modes.outputs['dry-run'] }}"),
        "release workflow should capture dry-run output"
    );
    assert!(
        contents.contains("should_upload_workflow_artifacts: ${{ steps.release_modes.outputs['should-upload-workflow-artifacts'] }}"),
        "release workflow should capture workflow artefact upload output"
    );
}

#[test]
fn behavioural_release_workflow_requires_pinned_canaries() -> Result<()> {
    let contents = workflow_contents("release.yml")?;
    let workflow: YamlValue =
        serde_yaml::from_str(&contents).context("parse release workflow YAML")?;
    let admission_command = release_admission_command(&workflow)?;
    let admission_script = release_admission_script()?;

    require_release_admission_workflow_wiring(&contents, admission_command)?;
    require_pinned_canary_revisions(&admission_script)?;
    require_exact_revision_run_lookup(&admission_script)?;
    require_successful_trusted_run_evidence(&admission_script)?;

    Ok(())
}

#[rstest]
#[case("linux-x86_64")]
#[case("linux-aarch64")]
fn behavioural_release_workflow_passes_linux_stage_targets(#[case] target_key: &str) {
    let contents = workflow_contents("release.yml").expect("release workflow should be readable");

    assert!(
        contents.contains(&format!("target_key: {target_key}")),
        "release workflow should declare Linux stage target {target_key}"
    );
    assert!(
        contents.contains("stage-target: ${{ matrix.target_key }}"),
        "release workflow should pass matrix stage targets to build-and-package"
    );
}
