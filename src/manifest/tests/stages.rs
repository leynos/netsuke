//! Tests for manifest stage callback ordering.

use crate::manifest::{self, ManifestLoadStage};
use crate::stdlib::NetworkPolicy;
use anyhow::{Context, Result, ensure};

/// Create a temporary workspace with a manifest containing the given command.
fn temp_manifest(command: &str) -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    let dir = tempfile::tempdir().context("create temp workspace for stage test")?;
    let manifest_path = dir.path().join("Netsukefile");
    std::fs::write(
        &manifest_path,
        format!(
            concat!(
                "netsuke_version: \"1.0.0\"\n",
                "rules:\n",
                "  - name: touch\n",
                "    command: \"{command}\"\n",
                "targets:\n",
                "  - name: out.txt\n",
                "    rule: touch\n",
            ),
            command = command,
        ),
    )
    .with_context(|| format!("write {}", manifest_path.display()))?;
    Ok((dir, manifest_path))
}

#[test]
fn stage_callback_reports_expected_order_for_valid_manifest() -> Result<()> {
    let (_dir, manifest_path) = temp_manifest("touch $out")?;

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

/// Template expansion failures should report stages up to and including
/// `TemplateExpansion`.
#[test]
fn stage_callback_stops_after_template_expansion_failure() -> Result<()> {
    let dir = tempfile::tempdir().context("create temp workspace for template test")?;
    let manifest_path = dir.path().join("Netsukefile");
    // Reference a non-existent Jinja variable to trigger expansion failure.
    std::fs::write(
        &manifest_path,
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "rules:\n",
            "  - name: echo\n",
            "    command: \"echo hello\"\n",
            "targets:\n",
            "  - name: out.txt\n",
            "    rule: echo\n",
            "    when: \"{{ nonexistent_variable }}\"\n",
        ),
    )
    .with_context(|| format!("write {}", manifest_path.display()))?;

    let mut stages = Vec::new();
    let result = manifest::from_path_with_policy_and_stage_callback(
        &manifest_path,
        NetworkPolicy::default(),
        |stage| stages.push(stage),
    );
    ensure!(result.is_err(), "template expansion should fail");
    ensure!(
        stages.contains(&ManifestLoadStage::TemplateExpansion),
        "stages should include TemplateExpansion: {stages:?}"
    );
    ensure!(
        !stages.contains(&ManifestLoadStage::FinalRendering),
        "stages should not include FinalRendering: {stages:?}"
    );
    Ok(())
}

/// Final rendering failures should report stages up to and including
/// `FinalRendering`.
#[test]
fn stage_callback_stops_after_final_rendering_failure() -> Result<()> {
    let dir = tempfile::tempdir().context("create temp workspace for rendering test")?;
    let manifest_path = dir.path().join("Netsukefile");
    // Missing required `netsuke_version` causes a deserialization error during
    // FinalRendering.
    std::fs::write(
        &manifest_path,
        concat!(
            "rules:\n",
            "  - name: echo\n",
            "    command: \"echo hello\"\n",
            "targets:\n",
            "  - name: out.txt\n",
            "    rule: echo\n",
        ),
    )
    .with_context(|| format!("write {}", manifest_path.display()))?;

    let mut stages = Vec::new();
    let result = manifest::from_path_with_policy_and_stage_callback(
        &manifest_path,
        NetworkPolicy::default(),
        |stage| stages.push(stage),
    );
    ensure!(result.is_err(), "final rendering should fail");
    ensure!(
        stages.contains(&ManifestLoadStage::FinalRendering),
        "stages should include FinalRendering: {stages:?}"
    );
    Ok(())
}
