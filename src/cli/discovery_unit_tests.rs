//! Unit tests for config discovery through injected environment access.

use super::*;
use crate::cli::test_support::TestEnv;
use anyhow::{Context, ensure};
use cap_std::{ambient_authority, fs::Dir};
use rstest::rstest;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn env_config_path_returns_none_when_var_unset() {
    let env = TestEnv::default();
    assert!(env_config_path(&env, "__NETSUKE_TEST_VAR").is_none());
}

#[test]
fn env_config_path_returns_none_when_var_empty() {
    let env = TestEnv::default().with_var("__NETSUKE_TEST_VAR", "");
    assert!(env_config_path(&env, "__NETSUKE_TEST_VAR").is_none());
}

#[test]
fn env_config_path_returns_path_when_var_set() {
    let env = TestEnv::default().with_var("__NETSUKE_TEST_VAR", "/tmp/foo.toml");
    let result = env_config_path(&env, "__NETSUKE_TEST_VAR");
    assert_eq!(result, Some(PathBuf::from("/tmp/foo.toml")));
}

#[rstest]
#[case::cli_wins_over_env(Some("/env/path.toml"), Some("/cli/path.toml"), Some("/cli/path.toml"))]
#[case::env_used_without_cli(Some("/env/path.toml"), None, Some("/env/path.toml"))]
#[case::none_when_sources_missing(None, None, None)]
fn explicit_config_path_obeys_precedence(
    #[case] env_path: Option<&'static str>,
    #[case] cli_path: Option<&'static str>,
    #[case] expected: Option<&'static str>,
) {
    let mut env = TestEnv::default();
    if let Some(path) = env_path {
        env = env.with_var(CONFIG_ENV_VAR, path);
    }
    let cli = Cli {
        config: cli_path.map(PathBuf::from),
        ..Cli::default()
    };

    assert_eq!(
        explicit_config_path_with_env(&cli, &env),
        expected.map(PathBuf::from)
    );
}

#[test]
fn collect_diag_file_layers_uses_injected_explicit_config() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let config_path = dir.path().join("netsuke.toml");
    let config_dir = Dir::open_ambient_dir(dir.path(), ambient_authority())?;
    config_dir.write("netsuke.toml", b"json = true\n")?;

    let env = TestEnv::default().with_var(CONFIG_ENV_VAR, config_path.as_os_str());
    let discovered = collect_diag_file_layers_with_env(&Cli::default(), &env);
    // `load_config_file_as_chain` canonicalises the layer path through the
    // same normalizer discovery uses, so compare the injected path in that
    // canonical form. On Windows this folds short-name and UNC-prefixed
    // spellings into the long-name form the layer records.
    let expected_path =
        paths::normalized_path_key(&paths::FsPathNormalizer, &config_path.to_string_lossy())
            .context("canonicalise injected config path")?
            .to_string_lossy()
            .into_owned();

    ensure!(
        discovered.layers().iter().any(|layer| layer
            .path()
            .is_some_and(|path| path.as_str() == expected_path)),
        "should include the injected explicit config layer at {expected_path}"
    );

    Ok(())
}
