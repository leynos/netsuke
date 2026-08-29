//! Tests for explicit configuration-selector independence from `-C`.
//!
//! A relative `--config` or `NETSUKE_CONFIG` selector resolves from the
//! process working directory even when `-C/--directory` is supplied; an
//! absolute selector is always used unchanged. End-to-end tests place a
//! decoy at the `-C` location to prove the selector is not rebased.
use super::paths::{FsPathNormalizer, normalized_path_key};
use super::*;
use crate::cli::test_support::TestEnv;
use anyhow::{Context, Result, ensure};
use pretty_assertions::assert_eq;
use tempfile::tempdir;

/// Assert that `discovered` loaded exactly `expected_path` and nothing else.
fn assert_single_layer(
    discovered: &DiscoveryOutcome,
    expected_path: &std::path::Path,
) -> Result<()> {
    ensure!(
        discovered.first_error().is_none(),
        "the explicit selector should load: {:?}",
        discovered.first_error()
    );
    let paths = discovered
        .layers()
        .iter()
        .filter_map(|layer| layer.path().map(|path| path.as_str().to_owned()))
        .collect::<Vec<_>>();
    let expected = normalized_path_key(&FsPathNormalizer, &expected_path.to_string_lossy())
        .context("canonicalise the expected selector path")?
        .to_string_lossy()
        .into_owned();
    assert_eq!(paths, vec![expected]);
    Ok(())
}

/// An absolute `--config` selector is used unchanged, even with `-C`.
///
/// `-C/--directory` never re-anchors an absolute selector: the operator
/// pointed at an exact file. A decoy at the `-C`-joined path with different
/// content proves that rebasing an absolute selector would be caught.
#[test]
fn explicit_absolute_config_ignores_cli_directory() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let selector = temp.path().join("selector.toml");
    test_support::fs::write(&selector, "theme = \"ascii\"\n").context("write selector config")?;
    let cli_directory = temp.path().join("cli-dir");
    test_support::fs::create_dir(&cli_directory).context("create -C directory")?;
    // A decoy at the `-C`-joined path: if the selector were anchored to `-C`,
    // this is what would actually load instead of `selector`.
    test_support::fs::write(cli_directory.join("selector.toml"), "theme = \"dark\"\n")
        .context("write -C decoy config")?;

    let cli = Cli {
        config: Some(selector.clone()),
        directory: Some(cli_directory),
        ..Cli::default()
    };
    let discovered = discover_file_layers(&cli, &TestEnv::default());

    assert_single_layer(&discovered, &selector)
}

/// A relative `--config` selector keeps its process-working-directory spelling.
#[test]
fn explicit_relative_config_ignores_cli_directory() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let cli_directory = temp.path().join("cli-dir");
    test_support::fs::create_dir(&cli_directory).context("create -C directory")?;

    let cli = Cli {
        config: Some(PathBuf::from("relative.toml")),
        directory: Some(cli_directory.clone()),
        ..Cli::default()
    };

    assert_eq!(
        explicit_config_path_with_env(&cli, &TestEnv::default()),
        Some(PathBuf::from("relative.toml"))
    );
    Ok(())
}

/// A relative `--config` selector without `-C` is returned unchanged.
///
/// The pure query keeps the spelling as written; at load time it then
/// resolves against the process working directory, which the end-to-end
/// binary coverage in `tests/config_discovery_e2e_tests.rs` proves.
#[test]
fn explicit_relative_config_without_directory_stays_as_written() {
    let cli = Cli {
        config: Some(PathBuf::from("relative.toml")),
        ..Cli::default()
    };
    assert_eq!(
        explicit_config_path_with_env(&cli, &TestEnv::default()),
        Some(PathBuf::from("relative.toml"))
    );
}

/// `--config` keeps precedence over `NETSUKE_CONFIG` regardless of `-C`.
#[test]
fn cli_selector_wins_over_environment_with_directory() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let cli_directory = temp.path().join("cli-dir");
    test_support::fs::create_dir(&cli_directory).context("create -C directory")?;

    let cli = Cli {
        config: Some(PathBuf::from("cli.toml")),
        directory: Some(cli_directory.clone()),
        ..Cli::default()
    };
    let env = TestEnv::default().with_var(CONFIG_ENV_VAR, "env.toml");

    assert_eq!(
        explicit_config_path_with_env(&cli, &env),
        Some(PathBuf::from("cli.toml"))
    );
    Ok(())
}

/// A relative `NETSUKE_CONFIG` selector also ignores `-C`.
#[test]
fn env_config_selector_ignores_cli_directory() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let cli_directory = temp.path().join("cli-dir");
    test_support::fs::create_dir(&cli_directory).context("create -C directory")?;

    let cli = Cli {
        directory: Some(cli_directory.clone()),
        ..Cli::default()
    };
    let env = TestEnv::default().with_var(CONFIG_ENV_VAR, "env-selector.toml");

    assert_eq!(
        explicit_config_path_with_env(&cli, &env),
        Some(PathBuf::from("env-selector.toml"))
    );
    Ok(())
}
