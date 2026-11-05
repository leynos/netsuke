//! Preserve diagnostic JSON precedence when quarantined policy validation fails.

use super::resolve_json_and_layers_outcome_with_env;
use crate::cli::{Cli, merge_with_cached_file_layers, test_support::TestEnv};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::{CommandFactory, Parser};
use ortho_config::OrthoError;
use rstest::rstest;
use tempfile::tempdir;

/// Identify the source selecting diagnostic mode above the malformed file.
#[derive(Clone, Copy, Debug)]
enum JsonOverride {
    File,
    Environment,
    CommandLine,
}

/// Write an invalid fetch-policy request in the primary file or an inherited file.
fn write_project_policy(dir: &Dir, field: &str, inherited: bool) -> Result<()> {
    if inherited {
        dir.write("policy.toml", format!("{field} = \"true\"\n"))?;
        dir.write(".netsuke.toml", "json = true\nextends = \"policy.toml\"\n")?;
    } else {
        dir.write(
            ".netsuke.toml",
            format!("json = true\n{field} = \"true\"\n"),
        )?;
    }
    Ok(())
}

/// Verify one malformed request preserves JSON precedence and its typed error.
///
/// # Errors
///
/// Returns an error when discovery or merge loses the malformed field's typed
/// validation error, or when the selected JSON source does not win.
fn assert_malformed_policy_case(
    field: &str,
    inherited: bool,
    json_override: JsonOverride,
) -> Result<()> {
    let project = tempdir()?;
    let config_dir = Dir::open_ambient_dir(project.path(), ambient_authority())?;
    write_project_policy(&config_dir, field, inherited)?;
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
    let env = match json_override {
        JsonOverride::File => base_env,
        JsonOverride::Environment | JsonOverride::CommandLine => {
            base_env.with_var("NETSUKE_JSON", "false")
        }
    };

    let (json, outcome) = resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    ensure!(
        json? != matches!(json_override, JsonOverride::Environment),
        "the selected JSON source should win for {json_override:?}"
    );
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

#[rstest]
#[case::default_deny("fetch_default_deny")]
#[case::allow_scheme("fetch_allow_scheme")]
#[case::allow_host("fetch_allow_host")]
#[case::env_allow_var("env_allow_var")]
#[case::project_trust("trust_project_fetch_policy")]
fn malformed_project_policy_preserves_json_and_validation_error(#[case] field: &str) -> Result<()> {
    for inherited in [false, true] {
        for json_override in [
            JsonOverride::File,
            JsonOverride::Environment,
            JsonOverride::CommandLine,
        ] {
            assert_malformed_policy_case(field, inherited, json_override)?;
        }
    }
    Ok(())
}

/// Report whether `error`, or any error it aggregates, is a validation fault.
///
/// Discovery and merge can each report the same rejected field, and
/// `LayerComposition::into_merge_result` aggregates them rather than choosing
/// one, so a direct `matches!` would miss the discovery error.
fn has_validation_key(error: Option<&OrthoError>, key: &str) -> bool {
    match error {
        Some(OrthoError::Validation { key: actual, .. }) => actual == key,
        Some(OrthoError::Aggregate(aggregated)) => aggregated
            .iter()
            .any(|inner| has_validation_key(Some(inner), key)),
        _ => false,
    }
}

/// Verify one rejected shell declaration still defers to a later `json = true`.
///
/// The two cases exercise different authority classes: an untrusted project
/// source is rejected at the bare `shells` key, while a trusted operator source
/// is rejected at the malformed field's key. They share every other step, so
/// keeping them in step matters if the resolution API changes.
#[rstest]
#[case::project_source_is_untrusted(
    ".netsuke.toml",
    concat!(
        "json = true\n",
        "[[shells]]\n",
        "name = \"dash\"\n",
        "executable = \"dash\"\n",
        "args = [\"-c\"]\n"
    ),
    "shells",
    "an untrusted project definition"
)]
#[case::operator_definition_is_malformed(
    "user/netsuke/config.toml",
    concat!(
        "json = true\n",
        "[[shells]]\n",
        "name = \"dash\"\n",
        "executable = \"bin/dash\"\n",
        "args = [\"-c\"]\n"
    ),
    "shells[0].executable",
    "a malformed operator definition"
)]
fn shell_fault_still_honours_later_json_preference(
    #[case] relative: &str,
    #[case] body: &str,
    #[case] key: &str,
    #[case] authority: &str,
) -> Result<()> {
    let project = tempdir()?;
    let config_dir = Dir::open_ambient_dir(project.path(), ambient_authority())?;
    // Only the operator file sits in a subdirectory; creating the root on the
    // project file's behalf would be an unnecessary capability request.
    if let Some((parent, _)) = relative.rsplit_once('/') {
        config_dir.create_dir_all(parent)?;
    }
    config_dir.write(relative, body)?;
    let project_path = project.path().to_str().context("project path is UTF-8")?;
    let args = vec!["netsuke", "--directory", project_path];
    let cli = Cli::parse_from(&args);
    let matches = Cli::command().get_matches_from(&args);
    let env = TestEnv::default()
        .with_var("HOME", project.path())
        .with_var("XDG_CONFIG_HOME", project.path().join("user"))
        .with_var("XDG_CONFIG_DIRS", project.path().join("system"));

    let (json, outcome) = resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    ensure!(json?, "{authority} should still honour `json = true`");
    ensure!(
        has_validation_key(outcome.first_error().map(AsRef::as_ref), key),
        "discovery must retain the validation error for {key}"
    );
    let error = merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers())
        .expect_err("cached merge must reject the rejected shells declaration");
    ensure!(
        has_validation_key(Some(error.as_ref()), key),
        "merge must retain the validation error for {key}: {error:?}"
    );
    Ok(())
}
