//! Test explicit-selector resolution and project-policy discovery boundaries.
//!
//! These discovery tests show that relative selectors resolve from the
//! process working directory and absolute selectors remain unchanged when
//! `-C/--directory` is supplied. They also verify that selected project-chain
//! policy fields are quarantined while unrelated operator configuration keeps
//! its authority.
use super::super::paths::{FsPathNormalizer, normalized_path_key};
use super::super::*;
use crate::cli::test_support::TestEnv;
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
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
    let cli_directory_path = Utf8PathBuf::from_path_buf(cli_directory).map_err(|non_utf8| {
        anyhow::anyhow!("CLI directory is not valid UTF-8: {}", non_utf8.display())
    })?;

    let cli = Cli {
        config: Some(selector.clone()),
        directory: Some(cli_directory_path),
        ..Cli::default()
    };
    let discovered = discover_file_layers(&cli, &TestEnv::default());

    assert_single_layer(&discovered, &selector)
}

#[test]
fn explicit_project_selector_quarantines_its_inherited_chain() -> Result<()> {
    let temp = tempdir().context("create project directory")?;
    let inherited = temp.path().join("policy.toml");
    let primary = temp.path().join(".netsuke.toml");
    test_support::fs::write(
        &inherited,
        "fetch_allow_host = [\"169.254.169.254\"]\ntrust_project_fetch_policy = true\n",
    )
    .context("write inherited project policy")?;
    test_support::fs::write(
        &primary,
        "extends = \"policy.toml\"\nfetch_allow_scheme = [\"http\"]\n",
    )
    .context("write explicit project selector")?;
    let cli = Cli {
        config: Some(primary),
        directory: Some(
            Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).map_err(|non_utf8| {
                anyhow::anyhow!(
                    "project directory is not valid UTF-8: {}",
                    non_utf8.display()
                )
            })?,
        ),
        ..Cli::default()
    };

    let (layers, errors, project_request, _) = discover_file_layers(&cli, &TestEnv::default())
        .into_layers()
        .into_parts();

    ensure!(
        errors.is_empty(),
        "explicit project chain should load: {errors:?}"
    );
    let request = project_request.context("explicit project chain should be quarantined")?;
    ensure!(
        request.allow_scheme == ["http"],
        "project scheme should be captured"
    );
    ensure!(
        request.allow_host == [crate::host_pattern::HostPattern::parse("169.254.169.254")?],
        "project host should be captured"
    );
    for layer in layers {
        let value = layer.into_value();
        ensure!(
            value.get("fetch_allow_scheme").is_none()
                && value.get("fetch_allow_host").is_none()
                && value.get("trust_project_fetch_policy").is_none(),
            "project-chain grants and trust should be quarantined"
        );
    }
    Ok(())
}

#[test]
fn unrelated_explicit_operator_selector_keeps_ordinary_file_fields() -> Result<()> {
    let temp = tempdir().context("create project directory")?;
    test_support::fs::write(
        temp.path().join(".netsuke.toml"),
        "fetch_default_deny = true\n",
    )
    .context("write automatically discoverable project config")?;
    let selector = temp.path().join("operator.toml");
    test_support::fs::write(&selector, "fetch_allow_scheme = [\"http\"]\n")
        .context("write unrelated explicit config")?;
    let cli = Cli {
        config: Some(selector),
        directory: Some(
            Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).map_err(|non_utf8| {
                anyhow::anyhow!(
                    "project directory is not valid UTF-8: {}",
                    non_utf8.display()
                )
            })?,
        ),
        ..Cli::default()
    };

    let (layers, errors, request, _) = discover_file_layers(&cli, &TestEnv::default())
        .into_layers()
        .into_parts();

    ensure!(
        errors.is_empty(),
        "explicit operator config should load: {errors:?}"
    );
    ensure!(
        request.is_none(),
        "unrelated config has no project occurrence"
    );
    let layer = layers
        .into_iter()
        .next()
        .context("explicit operator layer should be retained")?;
    let value = layer.into_value();
    ensure!(
        value.get("fetch_allow_scheme") == Some(&serde_json::json!(["http"])),
        "unrelated explicit configuration should retain ordinary fields"
    );
    Ok(())
}

/// Preserve selector precedence and process-directory resolution under `-C`.
#[rstest::rstest]
#[case::relative_cli_selector_without_environment(Some("relative.toml"), None, "relative.toml")]
#[case::cli_selector_wins_over_environment(Some("cli.toml"), Some("env.toml"), "cli.toml")]
#[case::environment_selector_ignores_cli_directory(
    None,
    Some("env-selector.toml"),
    "env-selector.toml"
)]
fn explicit_selector_ignores_cli_directory(
    #[case] cli_selector: Option<&str>,
    #[case] env_selector: Option<&str>,
    #[case] expected_selector: &str,
) -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let cli_directory = temp.path().join("cli-dir");
    test_support::fs::create_dir(&cli_directory).context("create -C directory")?;
    let cli_directory_path = Utf8PathBuf::from_path_buf(cli_directory).map_err(|non_utf8| {
        anyhow::anyhow!("CLI directory is not valid UTF-8: {}", non_utf8.display())
    })?;

    let cli = Cli {
        config: cli_selector.map(PathBuf::from),
        directory: Some(cli_directory_path),
        ..Cli::default()
    };
    let env = env_selector.map_or_else(TestEnv::default, |selector| {
        TestEnv::default().with_var(CONFIG_ENV_VAR, selector)
    });

    assert_eq!(
        explicit_config_path_with_env(&cli, &env),
        Some(PathBuf::from(expected_selector))
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
