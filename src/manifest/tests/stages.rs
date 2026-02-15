//! Tests for manifest stage callback ordering.

use crate::manifest::{self, ManifestLoadStage};
use crate::stdlib::NetworkPolicy;
use anyhow::{Context, Result, ensure};
use rstest::{fixture, rstest};

/// Create a temporary workspace with a manifest path but no file content.
#[fixture]
fn temp_workspace() -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    let dir = tempfile::tempdir().context("create temp workspace for stage test")?;
    let manifest_path = dir.path().join("Netsukefile");
    Ok((dir, manifest_path))
}

#[rstest]
fn stage_callback_reports_expected_order_for_valid_manifest(
    temp_workspace: Result<(tempfile::TempDir, std::path::PathBuf)>,
) -> Result<()> {
    let (_dir, manifest_path) = temp_workspace?;
    std::fs::write(
        &manifest_path,
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "rules:\n",
            "  - name: touch\n",
            "    command: \"touch $out\"\n",
            "targets:\n",
            "  - name: out.txt\n",
            "    rule: touch\n",
        ),
    )
    .with_context(|| format!("write {}", manifest_path.display()))?;

    let mut stages = Vec::new();
    let manifest = manifest::from_path_with_policy(
        &manifest_path,
        NetworkPolicy::default(),
        Some(&mut |stage| stages.push(stage)),
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

#[rstest]
fn stage_callback_stops_after_parse_failure(
    temp_workspace: Result<(tempfile::TempDir, std::path::PathBuf)>,
) -> Result<()> {
    let (_dir, manifest_path) = temp_workspace?;
    std::fs::write(&manifest_path, "targets:\n\t- name: broken\n")
        .with_context(|| format!("write {}", manifest_path.display()))?;

    let mut stages = Vec::new();
    let result = manifest::from_path_with_policy(
        &manifest_path,
        NetworkPolicy::default(),
        Some(&mut |stage| stages.push(stage)),
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
#[rstest]
fn stage_callback_stops_after_template_expansion_failure(
    temp_workspace: Result<(tempfile::TempDir, std::path::PathBuf)>,
) -> Result<()> {
    let (_dir, manifest_path) = temp_workspace?;
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
    let result = manifest::from_path_with_policy(
        &manifest_path,
        NetworkPolicy::default(),
        Some(&mut |stage| stages.push(stage)),
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
#[rstest]
fn stage_callback_stops_after_final_rendering_failure(
    temp_workspace: Result<(tempfile::TempDir, std::path::PathBuf)>,
) -> Result<()> {
    let (_dir, manifest_path) = temp_workspace?;
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
    let result = manifest::from_path_with_policy(
        &manifest_path,
        NetworkPolicy::default(),
        Some(&mut |stage| stages.push(stage)),
    );
    ensure!(result.is_err(), "final rendering should fail");
    ensure!(
        stages.contains(&ManifestLoadStage::FinalRendering),
        "stages should include FinalRendering: {stages:?}"
    );
    Ok(())
}
