//! End-to-end configuration discovery failure coverage.
//!
//! These tests run the real binary in a child process with a closed
//! environment, proving that missing configuration still permits the normal
//! workflow while a malformed discovered file fails before manifest handling.

use anyhow::{Context, Result, ensure};
use assert_cmd::cargo::cargo_bin_cmd;
use camino::{Utf8Path, Utf8PathBuf};
use proptest::prelude::*;
use serde_json::Value;
use tempfile::{TempDir, tempdir};
use test_support::fs as test_fs;

#[derive(Clone, Copy, Debug)]
enum ExplicitSelector {
    Cli,
    Environment,
}

#[derive(Clone, Copy, Debug)]
enum SelectorPathKind {
    Relative,
    Absolute,
}

fn workspace(context: &str) -> Result<TempDir> {
    let temp = tempdir().with_context(|| format!("create workspace for {context}"))?;
    test_fs::copy("tests/data/minimal.yml", temp.path().join("Netsukefile"))
        .with_context(|| format!("write manifest for {context}"))?;
    Ok(temp)
}

fn isolated_netsuke_command(current_dir: &Utf8Path) -> assert_cmd::Command {
    let mut command = cargo_bin_cmd!("netsuke");
    command.current_dir(current_dir.as_std_path()).env_clear();
    command
}

fn utf8_workspace_path(temp: &TempDir) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("workspace path {} is not UTF-8", path.display()))
}

#[test]
fn no_discovered_config_allows_the_graph_workflow() -> Result<()> {
    let temp = workspace("no discovered configuration")?;
    let workspace_path = utf8_workspace_path(&temp)?;
    let output = isolated_netsuke_command(&workspace_path)
        .arg("graph")
        .output()
        .context("run graph without configuration")?;

    ensure!(output.status.success(), "graph should use defaults");
    Ok(())
}

#[test]
fn malformed_discovered_config_fails_the_binary_workflow() -> Result<()> {
    let temp = workspace("malformed discovered configuration")?;
    test_fs::write(temp.path().join(".netsuke.toml"), "emoji = \"always\n")
        .context("write malformed discovered config")?;

    let workspace_path = utf8_workspace_path(&temp)?;
    let output = isolated_netsuke_command(&workspace_path)
        .arg("graph")
        .output()
        .context("run graph with malformed configuration")?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    ensure!(!output.status.success(), "malformed config must fail");
    ensure!(
        stderr.contains(".netsuke.toml"),
        "configuration failure should identify the discovered file: {stderr}",
    );
    Ok(())
}

/// Assert that an explicit selector does not rebase beneath `-C`.
fn assert_explicit_relative_config_ignores_directory_anchor(
    selector: ExplicitSelector,
    selector_path_kind: SelectorPathKind,
    project_name: &str,
    config_name: &str,
) -> Result<()> {
    let outer = tempdir().context("create invoking directory")?;
    let project = outer.path().join(project_name);
    test_fs::create_dir(&project).context("create directory-anchored project")?;
    test_fs::copy("tests/data/minimal.yml", project.join("Netsukefile"))
        .context("write project manifest")?;
    test_fs::write(outer.path().join(config_name), "json = true\n")
        .context("write invoking-directory config")?;
    test_fs::write(project.join(config_name), "json = false\n")
        .context("write directory-anchored config")?;

    let outer_path = utf8_workspace_path(&outer)?;
    let selector_path = match selector_path_kind {
        // Relative explicit selectors stay anchored to the child CWD.
        SelectorPathKind::Relative => Utf8PathBuf::from(config_name),
        // Absolute explicit selectors remain unchanged.
        SelectorPathKind::Absolute => outer_path.join(config_name),
    };
    let mut command = isolated_netsuke_command(&outer_path);
    command.args(["-C", project_name]);
    match selector {
        ExplicitSelector::Cli => {
            command.arg("--config").arg(selector_path.as_str());
        }
        ExplicitSelector::Environment => {
            command.env("NETSUKE_CONFIG", selector_path.as_str());
        }
    }
    let output = command
        .arg("generate")
        .output()
        .context("run generate with an explicit config")?;

    ensure!(
        output.status.success(),
        "generate should succeed: {output:?}"
    );
    let document: Value = serde_json::from_slice(&output.stdout)
        .context("explicit selected config should enable JSON output")?;
    ensure!(
        document
            .pointer("/result/content")
            .and_then(Value::as_str)
            .is_some(),
        "JSON output should contain the generated Ninja artefact: {document}",
    );
    Ok(())
}

/// An explicit relative CLI selector stays anchored to the child process CWD.
#[test]
fn cli_explicit_relative_config_ignores_directory_anchor() -> Result<()> {
    assert_explicit_relative_config_ignores_directory_anchor(
        ExplicitSelector::Cli,
        SelectorPathKind::Relative,
        "project",
        "relative.toml",
    )
}

/// A relative environment selector stays anchored to the child process CWD.
#[test]
fn environment_explicit_relative_config_ignores_directory_anchor() -> Result<()> {
    assert_explicit_relative_config_ignores_directory_anchor(
        ExplicitSelector::Environment,
        SelectorPathKind::Relative,
        "project",
        "relative.toml",
    )
}

/// An absolute CLI selector remains unchanged when `-C` is present.
#[test]
fn cli_explicit_absolute_config_ignores_directory_anchor() -> Result<()> {
    assert_explicit_relative_config_ignores_directory_anchor(
        ExplicitSelector::Cli,
        SelectorPathKind::Absolute,
        "project",
        "absolute.toml",
    )
}

/// An absolute environment selector remains unchanged when `-C` is present.
#[test]
fn environment_explicit_absolute_config_ignores_directory_anchor() -> Result<()> {
    assert_explicit_relative_config_ignores_directory_anchor(
        ExplicitSelector::Environment,
        SelectorPathKind::Absolute,
        "project",
        "absolute.toml",
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Generated selector paths preserve explicit-selection anchoring.
    #[test]
    fn explicit_config_never_rebases_under_directory(
        selector in prop_oneof![
            Just(ExplicitSelector::Cli),
            Just(ExplicitSelector::Environment),
        ],
        selector_path_kind in prop_oneof![
            Just(SelectorPathKind::Relative),
            Just(SelectorPathKind::Absolute),
        ],
        project_name in "[a-z]{1,12}",
        config_stem in "[a-z]{1,12}",
    ) {
        let config_name = format!("{config_stem}.toml");
        let result = assert_explicit_relative_config_ignores_directory_anchor(
            selector,
            selector_path_kind,
            &project_name,
            &config_name,
        );
        prop_assert!(result.is_ok(), "{result:?}");
    }
}
