//! Preserve diagnostic JSON precedence when quarantined policy validation fails.

use super::resolve_json_and_layers_outcome_with_env;
use crate::cli::{Cli, merge_with_cached_file_layers, test_support::TestEnv};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::{CommandFactory, Parser};
use ortho_config::OrthoError;
use pretty_assertions::assert_eq;
use rstest::rstest;
use tempfile::tempdir;

/// Identify the source selecting diagnostic mode above the malformed file.
#[derive(Clone, Copy, Debug)]
enum JsonOverride {
    File,
    Environment,
    CommandLine,
}

/// Write an invalid fetch-policy request into an isolated project chain.
fn write_project_policy(dir: &Dir, field: &str, is_extended: bool) -> Result<()> {
    let malformed = format!("json = true\n{field} = \"true\"\n");
    if is_extended {
        dir.write("policy.toml", malformed)?;
        dir.write(".netsuke.toml", "extends = \"policy.toml\"\n")?;
    } else {
        dir.write(".netsuke.toml", malformed)?;
    }
    Ok(())
}

#[rstest]
#[case::default_deny("fetch_default_deny")]
#[case::allow_scheme("fetch_allow_scheme")]
#[case::allow_host("fetch_allow_host")]
#[case::project_trust("trust_project_fetch_policy")]
fn malformed_project_policy_preserves_json_and_validation_error(
    #[case] field: &str,
    #[values(false, true)] is_extended: bool,
    #[values(
        JsonOverride::File,
        JsonOverride::Environment,
        JsonOverride::CommandLine
    )]
    json_override: JsonOverride,
) -> Result<()> {
    let project = tempdir()?;
    let config_dir = Dir::open_ambient_dir(project.path(), ambient_authority())?;
    write_project_policy(&config_dir, field, is_extended)?;
    let project_path = project.path().to_str().context("project path is UTF-8")?;
    let mut args = vec!["netsuke", "--directory", project_path];
    if matches!(json_override, JsonOverride::CommandLine) {
        args.push("--json");
    }
    let cli = Cli::parse_from(&args);
    let matches = Cli::command().get_matches_from(&args);
    let base_env = TestEnv::default()
        .with_var("HOME", project.path())
        .with_var("XDG_CONFIG_HOME", project.path().join("user"))
        .with_var("XDG_CONFIG_DIRS", project.path().join("system"));
    let env = if matches!(json_override, JsonOverride::File) {
        base_env
    } else {
        base_env.with_var("NETSUKE_JSON", "false")
    };

    let (json, outcome) = resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);

    assert_eq!(json?, !matches!(json_override, JsonOverride::Environment));
    ensure!(
        matches!(outcome.first_error().map(AsRef::as_ref),
            Some(OrthoError::Validation { key, .. }) if key == field),
        "discovery must retain the typed validation error for {field}"
    );
    let error = merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers())
        .expect_err("cached merge must reject the malformed quarantined field");
    ensure!(
        matches!(error.as_ref(), OrthoError::Validation { key, .. } if key == field),
        "merge must return the typed validation error for {field}: {error:?}"
    );
    Ok(())
}

#[test]
fn primary_json_overrides_malformed_extended_layer_json() -> Result<()> {
    let project = tempdir()?;
    let config_dir = Dir::open_ambient_dir(project.path(), ambient_authority())?;
    write_project_policy(&config_dir, "fetch_default_deny", true)?;
    config_dir.write(".netsuke.toml", "extends = \"policy.toml\"\njson = false\n")?;
    let project_path = project.path().to_str().context("project path is UTF-8")?;
    let args = ["netsuke", "--directory", project_path];
    let cli = Cli::parse_from(args);
    let matches = Cli::command().get_matches_from(args);
    let env = TestEnv::default()
        .with_var("HOME", project.path())
        .with_var("XDG_CONFIG_HOME", project.path().join("user"))
        .with_var("XDG_CONFIG_DIRS", project.path().join("system"));

    let (json, outcome) = resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);

    ensure!(
        !json?,
        "later primary JSON preference must remain effective"
    );
    ensure!(
        outcome.first_error().is_some(),
        "extended validation error must remain pending"
    );
    Ok(())
}
