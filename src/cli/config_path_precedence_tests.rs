//! Property and unit tests for the explicit config-path selector precedence
//! enforced by [`super::explicit_config_path`] in `discovery.rs`.
//!
//! Three selectors are evaluated in priority order:
//!
//! 1. `cli.config` (CLI `--config` flag) - highest priority.
//! 2. [`super::CONFIG_ENV_VAR`] (`NETSUKE_CONFIG`) - mid priority.
//! 3. [`super::CONFIG_ENV_VAR_LEGACY`] (`NETSUKE_CONFIG_PATH`) - lowest priority.
//!
//! The [`resolve_config_path_precedence`] rstest suite exhaustively enumerates
//! all 2^3 = 8 selector presence states. The
//! [`resolve_config_path_obeys_precedence_invariant`] proptest asserts that for
//! any combination of generated optional paths the resolved path equals the
//! highest-priority present selector.

use super::*;
use proptest::prelude::*;
use rstest::rstest;
use std::{collections::HashMap, ffi::OsString, path::PathBuf};

#[derive(Default)]
struct TestEnv {
    values: HashMap<&'static str, OsString>,
}

impl TestEnv {
    fn with_var(mut self, name: &'static str, value: impl Into<OsString>) -> Self {
        self.values.insert(name, value.into());
        self
    }
}

impl EnvProvider for TestEnv {
    fn get(&self, key: &str) -> Option<OsString> {
        self.values.get(key).cloned()
    }
}

fn precedence_winner<'a>(
    cli_config: Option<&'a PathBuf>,
    env_config: Option<&'a PathBuf>,
    legacy_config: Option<&'a PathBuf>,
) -> Option<&'a PathBuf> {
    cli_config.or(env_config).or(legacy_config)
}

fn resolve_config_path_with_selectors(
    cli_config: Option<PathBuf>,
    env_config: Option<&PathBuf>,
    legacy_config: Option<&PathBuf>,
) -> Option<PathBuf> {
    let mut env = env_config.map_or_else(TestEnv::default, |value| {
        TestEnv::default().with_var(CONFIG_ENV_VAR, value.as_os_str())
    });
    if let Some(value) = legacy_config {
        env = env.with_var(CONFIG_ENV_VAR_LEGACY, value.as_os_str());
    }
    let cli = Cli {
        config: cli_config,
        ..Cli::default()
    };
    explicit_config_path_with_env(&cli, &env)
}

/// For selectors S1 (`cli.config`), S2 (`NETSUKE_CONFIG`), and S3
/// (`NETSUKE_CONFIG_PATH`), the resolved path must be S1 when present, else S2
/// when present, else S3 when present, else `None`. These cases exhaustively
/// enumerate the 2^3 selector presence states.
#[rstest]
#[case::all_absent(None, None, None, None)]
#[case::legacy_only(None, None, Some("/legacy/path.toml"), Some("/legacy/path.toml"))]
#[case::env_only(None, Some("/env/path.toml"), None, Some("/env/path.toml"))]
#[case::env_wins_over_legacy(
    None,
    Some("/env/path.toml"),
    Some("/legacy/path.toml"),
    Some("/env/path.toml")
)]
#[case::cli_only(Some("/cli/path.toml"), None, None, Some("/cli/path.toml"))]
#[case::cli_wins_over_legacy_alone(
    Some("/cli/path.toml"),
    None,
    Some("/legacy/path.toml"),
    Some("/cli/path.toml")
)]
#[case::cli_wins_over_env(
    Some("/cli/path.toml"),
    Some("/env/path.toml"),
    None,
    Some("/cli/path.toml")
)]
#[case::cli_wins_over_both_env_vars(
    Some("/cli/path.toml"),
    Some("/env/path.toml"),
    Some("/legacy/path.toml"),
    Some("/cli/path.toml")
)]
fn resolve_config_path_precedence(
    #[case] cli_config: Option<&'static str>,
    #[case] env_config: Option<&'static str>,
    #[case] legacy_config: Option<&'static str>,
    #[case] expected: Option<&'static str>,
) {
    let env_path = env_config.map(PathBuf::from);
    let legacy_path = legacy_config.map(PathBuf::from);
    assert_eq!(
        resolve_config_path_with_selectors(
            cli_config.map(PathBuf::from),
            env_path.as_ref(),
            legacy_path.as_ref(),
        ),
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
        legacy_config in path_selector(),
    ) {
        let expected = precedence_winner(
            cli_config.as_ref(),
            env_config.as_ref(),
            legacy_config.as_ref(),
        ).cloned();
        let actual = resolve_config_path_with_selectors(
            cli_config,
            env_config.as_ref(),
            legacy_config.as_ref(),
        );

        prop_assert_eq!(actual, expected);
    }
}
