//! Property and unit tests for the explicit config-path selector precedence
//! enforced by [`super::explicit_config_path`] in `discovery.rs`.
//!
//! The CLI `--config` selector takes precedence over `NETSUKE_CONFIG`. The
//! table test covers every presence state, while the property test checks the
//! same invariant over generated path values.

use super::*;
use proptest::prelude::*;
use rstest::rstest;
use std::path::PathBuf;
use test_support::{EnvVarGuard, env_lock::EnvLock};

fn precedence_winner<'a>(
    cli_config: Option<&'a PathBuf>,
    env_config: Option<&'a PathBuf>,
) -> Option<&'a PathBuf> {
    cli_config.or(env_config)
}

fn resolve_config_path_with_selectors(
    cli_config: Option<PathBuf>,
    env_config: Option<&PathBuf>,
) -> Option<PathBuf> {
    let _lock = EnvLock::acquire();
    let mut env_guards = vec![EnvVarGuard::remove(CONFIG_ENV_VAR)];
    if let Some(value) = env_config {
        env_guards.push(EnvVarGuard::set(CONFIG_ENV_VAR, value.as_os_str()));
    }
    let cli = Cli {
        config: cli_config,
        ..Cli::default()
    };
    let result = explicit_config_path(&cli);
    drop(env_guards);
    result
}

/// The resolved path is the CLI selector when present, otherwise the
/// environment selector, otherwise `None`.
#[rstest]
#[case::all_absent(None, None, None)]
#[case::env_only(None, Some("/env/path.toml"), Some("/env/path.toml"))]
#[case::cli_only(Some("/cli/path.toml"), None, Some("/cli/path.toml"))]
#[case::cli_wins_over_env(Some("/cli/path.toml"), Some("/env/path.toml"), Some("/cli/path.toml"))]
fn resolve_config_path_precedence(
    #[case] cli_config: Option<&'static str>,
    #[case] env_config: Option<&'static str>,
    #[case] expected: Option<&'static str>,
) {
    let env_path = env_config.map(PathBuf::from);
    assert_eq!(
        resolve_config_path_with_selectors(cli_config.map(PathBuf::from), env_path.as_ref()),
        expected.map(PathBuf::from),
    );
}

fn path_selector() -> impl Strategy<Value = Option<PathBuf>> {
    proptest::option::of("[A-Za-z0-9._/-]{1,64}".prop_map(PathBuf::from))
}

proptest! {
    #[test]
    fn resolve_config_path_obeys_precedence_invariant(
        cli_config in path_selector(),
        env_config in path_selector(),
    ) {
        let expected = precedence_winner(cli_config.as_ref(), env_config.as_ref()).cloned();
        let actual = resolve_config_path_with_selectors(cli_config, env_config.as_ref());

        prop_assert_eq!(actual, expected);
    }
}
