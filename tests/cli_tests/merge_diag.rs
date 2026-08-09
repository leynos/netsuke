//! Integration coverage for early JSON preference resolution.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use mockable::MockEnv;
use netsuke::cli_localization;
use std::{ffi::OsString, sync::Arc};
use tempfile::tempdir;

#[test]
fn resolve_merged_json_honours_injected_env() -> Result<()> {
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    let config_dir = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())
        .context("open temporary config directory")?;
    config_dir
        .write("netsuke.toml", b"json = false\n")
        .context("write netsuke.toml")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let config_arg = config_path.to_string_lossy().into_owned();
    let (cli, matches) =
        netsuke::cli::parse_with_localizer_from(["netsuke", "--config", &config_arg], &localizer)
            .context("parse CLI args for injected JSON env")?;
    let mut env = MockEnv::new();
    env.expect_os_string()
        .returning(|key| (key == "NETSUKE_JSON").then(|| OsString::from("1")));

    ensure!(
        netsuke::cli::resolve_merged_json_with_env(&cli, &matches, &env)?,
        "injected NETSUKE_JSON should override file config",
    );

    Ok(())
}

/// Cached layers keep the file values found during diagnostic resolution.
#[test]
fn cached_merge_does_not_reload_discovered_config() -> Result<()> {
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    let config_dir = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())
        .context("open temporary config directory")?;
    config_dir
        .write("netsuke.toml", b"jobs = 13\n")
        .context("write initial config")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let config_arg = config_path.to_string_lossy().into_owned();
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(
        ["netsuke", "--config", config_arg.as_str()],
        &localizer,
    )
    .context("parse CLI args for cached merge")?;
    let env = TestEnv::default();
    let (json, outcome) =
        netsuke::cli::resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    ensure!(!json?, "initial configuration should not enable JSON mode");

    config_dir
        .write("netsuke.toml", b"jobs = 29\n")
        .context("change config after discovery")?;
    let merged =
        netsuke::cli::merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers())?;

    ensure!(
        merged.jobs == Some(13),
        "cached merge should retain the initially discovered file value"
    );
    Ok(())
}
