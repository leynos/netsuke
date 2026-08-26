//! Tests for explicit configuration-selector independence from `-C`.
//!
//! `-C/--directory` anchors automatic discovery only. An explicit `--config`
//! or `NETSUKE_CONFIG` selector is used exactly as written, relative to the
//! shell original working directory; a decoy file at the `-C`-joined path
//! proves that a regression to `-C`-anchored selection would be caught.
use super::paths::{FsPathNormalizer, normalized_path_key};
use super::*;
use crate::cli::test_support::TestEnv;
use anyhow::{Context, Result, ensure};
use pretty_assertions::assert_eq;
use tempfile::tempdir;

/// An explicit `--config` selector is used as written, independent of `-C`.
///
/// `-C/--directory` anchors automatic discovery, not an explicit selector. A
/// decoy file at the `-C`-joined path with different content proves that a
/// regression to `-C`-anchored selection would be caught by the path and
/// content assertions.
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

    ensure!(
        discovered.first_error().is_none(),
        "the explicit selector should load"
    );
    let paths = discovered
        .layers()
        .iter()
        .filter_map(|layer| layer.path().map(|path| path.as_str().to_owned()))
        .collect::<Vec<_>>();
    let expected = normalized_path_key(&FsPathNormalizer, &selector.to_string_lossy())
        .context("canonicalise the selector path")?
        .to_string_lossy()
        .into_owned();
    assert_eq!(paths, vec![expected]);
    Ok(())
}

/// A relative `NETSUKE_CONFIG` selector is likewise independent of `-C`.
///
/// The environment selector goes through the same `collect_file_layers_with_env`
/// branch as `--config`, so the decoy proves the environment selector is not
/// redirected to the `-C` directory either.
#[test]
fn env_config_selector_ignores_cli_directory() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let selector = temp.path().join("env-selector.toml");
    test_support::fs::write(&selector, "theme = \"ascii\"\n")
        .context("write environment selector config")?;
    let cli_directory = temp.path().join("cli-dir");
    test_support::fs::create_dir(&cli_directory).context("create -C directory")?;
    test_support::fs::write(
        cli_directory.join("env-selector.toml"),
        "theme = \"dark\"\n",
    )
    .context("write -C decoy config")?;

    let cli = Cli {
        directory: Some(cli_directory),
        ..Cli::default()
    };
    let env = TestEnv::default().with_var(CONFIG_ENV_VAR, selector.as_os_str());
    let discovered = discover_file_layers(&cli, &env);

    ensure!(
        discovered.first_error().is_none(),
        "the environment selector should load"
    );
    let paths = discovered
        .layers()
        .iter()
        .filter_map(|layer| layer.path().map(|path| path.as_str().to_owned()))
        .collect::<Vec<_>>();
    let expected = normalized_path_key(&FsPathNormalizer, &selector.to_string_lossy())
        .context("canonicalise the environment selector path")?
        .to_string_lossy()
        .into_owned();
    assert_eq!(paths, vec![expected]);
    Ok(())
}
