//! Exercise project-chain budget authority and explicit-selector independence.

use super::*;
use crate::cli::test_support::TestEnv;
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use rstest::rstest;
use tempfile::tempdir;

/// Convert isolated test directories into CLI paths.
fn utf8_directory(path: &Path) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path.to_path_buf())
        .map_err(|_| anyhow::anyhow!("test directory must be UTF-8"))
}

#[rstest]
#[case::invalid_toml("[invalid")]
#[case::missing_parent("extends = \"missing.toml\"\n")]
#[case::invalid_budget("manifest_fuel = \"invalid\"\n")]
fn explicit_selector_ignores_broken_project_chain(#[case] project: &str) -> Result<()> {
    let temp = tempdir().context("create project directory")?;
    let selected = temp.path().join("selected.toml");
    test_support::fs::write(&selected, "manifest_fuel = 23\n")?;
    test_support::fs::write(temp.path().join(".netsuke.toml"), project)?;
    let cli = Cli {
        directory: Some(utf8_directory(temp.path())?),
        config: Some(selected),
        ..Cli::default()
    };

    let discovered = discover_file_layers(&cli, &TestEnv::default()).into_layers();
    ensure!(
        discovered.first_error().is_none(),
        "explicit config must load"
    );
    let (layers, _, _, request) = discovered.into_parts();
    ensure!(layers.len() == 1, "only the selected layer must load");
    ensure!(
        request.manifest_fuel.is_none(),
        "explicit budgets are operator-controlled"
    );
    ensure!(
        layers
            .into_iter()
            .next()
            .context("selected layer")?
            .into_value()
            .get("manifest_fuel")
            .and_then(serde_json::Value::as_u64)
            == Some(23),
        "selected budget must remain intact"
    );
    Ok(())
}

#[rstest]
#[case::inherited_widening(23, 17)]
#[case::inherited_narrowing(13, 13)]
fn nested_project_chain_reconciles_with_operator_limit(
    #[case] inherited: u64,
    #[case] expected: u64,
) -> Result<()> {
    let temp = tempdir().context("create project directory")?;
    test_support::fs::write(
        temp.path().join("base.toml"),
        format!("manifest_fuel = {inherited}\n"),
    )?;
    test_support::fs::write(temp.path().join("middle.toml"), "extends = \"base.toml\"\n")?;
    test_support::fs::write(
        temp.path().join(".netsuke.toml"),
        "extends = \"middle.toml\"\n",
    )?;
    let cli = Cli {
        directory: Some(utf8_directory(temp.path())?),
        ..Cli::default()
    };

    let discovered = discover_file_layers(&cli, &TestEnv::default()).into_layers();
    ensure!(
        discovered.first_error().is_none(),
        "project chain must load"
    );
    let (layers, _, _, request) = discovered.into_parts();
    ensure!(layers.len() == 3, "all three inherited layers must load");
    ensure!(
        layers
            .into_iter()
            .all(|layer| layer.into_value().get("manifest_fuel").is_none()),
        "project budgets must be quarantined from ordinary merging"
    );
    let operator = crate::cli::config::CliConfig {
        manifest_fuel: 17,
        ..crate::cli::config::CliConfig::default()
    };
    let resolved =
        crate::cli::manifest_budget_policy::reconcile_manifest_budget(operator, &request);
    ensure!(
        resolved.manifest_fuel == expected,
        "project budget must reconcile to {expected}"
    );
    Ok(())
}

#[rstest]
#[case::wrong_type("\"invalid\"")]
#[case::negative("-1")]
#[case::zero("0")]
fn inherited_invalid_budget_remains_an_error(#[case] invalid: &str) -> Result<()> {
    let temp = tempdir().context("create project directory")?;
    test_support::fs::write(
        temp.path().join("base.toml"),
        format!("manifest_fuel = {invalid}\n"),
    )?;
    test_support::fs::write(
        temp.path().join(".netsuke.toml"),
        "extends = \"base.toml\"\n",
    )?;
    let cli = Cli {
        directory: Some(utf8_directory(temp.path())?),
        ..Cli::default()
    };
    let discovered = discover_file_layers(&cli, &TestEnv::default());
    let error = discovered
        .first_error()
        .context("inherited invalid budget must fail")?;
    ensure!(
        matches!(error.as_ref(), ortho_config::OrthoError::Validation { key, .. } if key == "manifest_fuel"),
        "invalid inherited budget must retain its validation key"
    );
    Ok(())
}

#[test]
fn budget_quarantine_reuses_discovered_chain_metadata() -> Result<()> {
    let temp = tempdir().context("create project directory")?;
    let primary = temp.path().join(".netsuke.toml");
    test_support::fs::write(temp.path().join("base.toml"), "manifest_fuel = 13\n")?;
    test_support::fs::write(&primary, "extends = \"base.toml\"\n")?;
    let env = TestEnv::default();
    let (_, loaded) = layers::collect_file_layers_with_normalizer_and_trace(
        Some(temp.path()),
        &paths::FsPathNormalizer,
        discovery_env_source(&env),
    );
    let chain = loaded.context("discover project chain")?;
    test_support::fs::write(&primary, "extends = \"missing.toml\"\n")?;

    let resolved = layers::retain_layers_and_resolve_json(chain);
    ensure!(
        resolved.errors.is_empty(),
        "cached quarantine must not reload changed files"
    );
    ensure!(
        resolved.layers.len() == 2,
        "both cached layers must remain available"
    );
    ensure!(
        resolved.project_budget_request.manifest_fuel == Some(13),
        "cached inherited restriction must be retained"
    );
    Ok(())
}

/// Project-owned inherited configuration must not widen a manifest budget.
#[test]
fn project_extends_chain_contributes_only_narrower_budget_limits() -> Result<()> {
    let temp = tempdir().context("create project directory")?;
    test_support::fs::write(temp.path().join("base.toml"), "manifest_fuel = 17\n")
        .context("write inherited budget")?;
    test_support::fs::write(
        temp.path().join(".netsuke.toml"),
        "extends = \"base.toml\"\nmanifest_fuel = 19\n",
    )
    .context("write project configuration")?;

    let cli = Cli {
        directory: Some(utf8_directory(temp.path())?),
        ..Cli::default()
    };
    let discovered = discover_file_layers(&cli, &TestEnv::default()).into_layers();
    ensure!(
        discovered.first_error().is_none(),
        "project chain must load"
    );
    let (_, _, _, request) = discovered.into_parts();

    ensure!(
        request.manifest_fuel == Some(17),
        "inherited restriction must win"
    );
    Ok(())
}

/// Malformed project budget values must remain for the schema validator.
#[test]
fn malformed_project_budget_value_is_retained_for_validation() -> Result<()> {
    let temp = tempdir().context("create project directory")?;
    test_support::fs::write(
        temp.path().join(".netsuke.toml"),
        "manifest_fuel = \"not-a-number\"\n",
    )
    .context("write malformed project configuration")?;

    let cli = Cli {
        directory: Some(utf8_directory(temp.path())?),
        ..Cli::default()
    };
    let discovered = discover_file_layers(&cli, &TestEnv::default()).into_layers();
    ensure!(
        discovered.first_error().is_some(),
        "malformed budget must fail"
    );
    let (retained, _, _, request) = discovered.into_parts();
    let value = retained
        .into_iter()
        .next()
        .context("retain malformed project layer")?
        .into_value();

    ensure!(
        request.manifest_fuel.is_none(),
        "malformed value must not become a budget request"
    );
    value
        .get("manifest_fuel")
        .context("schema validation must receive the malformed project budget value")?;
    Ok(())
}
