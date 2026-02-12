//! Tests for manifest stage callback ordering.

use crate::manifest::{self, ManifestLoadStage};
use crate::stdlib::NetworkPolicy;
use anyhow::{Context, Result, ensure};

fn minimal_manifest() -> &'static str {
    concat!(
        "netsuke_version: \"1.0.0\"\n",
        "rules:\n",
        "  - name: touch\n",
        "    command: \"touch $out\"\n",
        "targets:\n",
        "  - name: out.txt\n",
        "    rule: touch\n",
    )
}

#[test]
fn stage_callback_reports_expected_order_for_valid_manifest() -> Result<()> {
    let dir = tempfile::tempdir().context("create temp workspace for stage test")?;
    let manifest_path = dir.path().join("Netsukefile");
    std::fs::write(&manifest_path, minimal_manifest())
        .with_context(|| format!("write {}", manifest_path.display()))?;

    let mut stages = Vec::new();
    let manifest = manifest::from_path_with_policy_and_stage_callback(
        &manifest_path,
        NetworkPolicy::default(),
        |stage| stages.push(stage),
    )?;

    ensure!(
        !manifest.targets.is_empty(),
        "manifest should contain targets"
    );
    ensure!(
        stages
            == vec![
                ManifestLoadStage::ManifestIngestion,
                ManifestLoadStage::InitialYamlParsing,
                ManifestLoadStage::TemplateExpansion,
                ManifestLoadStage::FinalRendering,
            ],
        "unexpected stage ordering: {stages:?}"
    );
    Ok(())
}

#[test]
fn stage_callback_stops_after_parse_failure() -> Result<()> {
    let dir = tempfile::tempdir().context("create temp workspace for stage failure test")?;
    let manifest_path = dir.path().join("Netsukefile");
    std::fs::write(&manifest_path, "targets:\n\t- name: broken\n")
        .with_context(|| format!("write {}", manifest_path.display()))?;

    let mut stages = Vec::new();
    let result = manifest::from_path_with_policy_and_stage_callback(
        &manifest_path,
        NetworkPolicy::default(),
        |stage| stages.push(stage),
    );
    ensure!(result.is_err(), "invalid manifest should fail");
    ensure!(
        stages
            == vec![
                ManifestLoadStage::ManifestIngestion,
                ManifestLoadStage::InitialYamlParsing,
            ],
        "unexpected stages for parse failure: {stages:?}"
    );
    Ok(())
}
