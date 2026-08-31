//! End-to-end configuration-discovery coverage through the real binary.
//!
//! These tests run `netsuke` in a child process with a closed environment and
//! an explicit invocation directory, proving the explicit-selector contract of
//! ADR-014: a relative `--config <PATH>` resolves against the process working
//! directory even when `-C/--directory` is supplied; an absolute selector is
//! unchanged. The
//! parent process environment and working directory are never mutated; all
//! child-process configuration flows through the `Command` builders.

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

/// Prove explicit-selector independence from `-C` through the binary.
///
/// The child runs from `invocation` with `-C project` and an explicit
/// selector. The invocation-directory copy enables early JSON output while
/// the `-C`-anchored copy does not, so the response shape names which file
/// loaded. A relative selector must load the invocation-directory copy;
/// an absolute selector must also remain unchanged.
fn assert_explicit_config_selection(
    selector: ExplicitSelector,
    selector_path_kind: SelectorPathKind,
    project_name: &str,
    config_name: &str,
) -> Result<()> {
    let invocation = tempdir().context("create invoking directory")?;
    let project = invocation.path().join(project_name);
    test_fs::create_dir(&project).context("create directory-anchored project")?;
    test_fs::copy("tests/data/minimal.yml", project.join("Netsukefile"))
        .context("write project manifest")?;
    // The invocation-directory copy wins for every explicit selector. The
    // `-C`-anchored copy is a decoy, while the invocation copy enables early
    // JSON output so the response shape names which file loaded.
    test_fs::write(invocation.path().join(config_name), "json = true\n")
        .context("write invocation-directory config")?;
    test_fs::write(project.join(config_name), "color = \"never\"\n")
        .context("write directory-anchored config")?;

    let invocation_path = utf8_workspace_path(&invocation)?;
    let selector_path = match selector_path_kind {
        // A relative explicit selector remains relative to the child CWD.
        SelectorPathKind::Relative => Utf8PathBuf::from(config_name),
        // An absolute explicit selector remains unchanged.
        SelectorPathKind::Absolute => invocation_path.join(config_name),
    };
    let mut command = isolated_netsuke_command(&invocation_path);
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
    let ninja = String::from_utf8_lossy(&output.stdout).into_owned();
    // The invocation-directory copy set `json = true`, so stdout is the JSON
    // envelope around the generated artefact. This proves `-C` did not rebase
    // the explicit relative selector onto its decoy.
    let document: Value = serde_json::from_str(&ninja)
        .with_context(|| format!("explicit selector should load the JSON config: {ninja}"))?;
    ensure!(
        document
            .pointer("/result/content")
            .and_then(Value::as_str)
            .is_some(),
        "JSON output should contain the generated Ninja artefact: {document}",
    );
    Ok(())
}

/// A relative CLI selector ignores `-C/--directory`.
#[test]
fn cli_explicit_relative_config_ignores_directory_anchor() -> Result<()> {
    assert_explicit_config_selection(
        ExplicitSelector::Cli,
        SelectorPathKind::Relative,
        "project",
        "relative.toml",
    )
}

/// A relative environment selector ignores `-C/--directory`.
#[test]
fn environment_explicit_relative_config_ignores_directory_anchor() -> Result<()> {
    assert_explicit_config_selection(
        ExplicitSelector::Environment,
        SelectorPathKind::Relative,
        "project",
        "relative.toml",
    )
}

/// An absolute CLI selector remains unchanged when `-C` is present.
#[test]
fn cli_explicit_absolute_config_ignores_directory_anchor() -> Result<()> {
    assert_explicit_config_selection(
        ExplicitSelector::Cli,
        SelectorPathKind::Absolute,
        "project",
        "absolute.toml",
    )
}

/// An absolute environment selector remains unchanged when `-C` is present.
#[test]
fn environment_explicit_absolute_config_ignores_directory_anchor() -> Result<()> {
    assert_explicit_config_selection(
        ExplicitSelector::Environment,
        SelectorPathKind::Absolute,
        "project",
        "absolute.toml",
    )
}

/// Without `-C`, a relative explicit selector resolves against the process
/// working directory.
#[test]
fn cli_explicit_relative_config_without_directory_uses_working_directory() -> Result<()> {
    let invocation = tempdir().context("create invoking directory")?;
    test_fs::copy(
        "tests/data/minimal.yml",
        invocation.path().join("Netsukefile"),
    )
    .context("write invocation manifest")?;
    test_fs::write(invocation.path().join("relative.toml"), "json = true\n")
        .context("write invocation-directory config")?;

    let invocation_path = utf8_workspace_path(&invocation)?;
    let output = isolated_netsuke_command(&invocation_path)
        .arg("--config")
        .arg("relative.toml")
        .arg("generate")
        .output()
        .context("run generate with an unanchored relative config")?;

    ensure!(
        output.status.success(),
        "generate should succeed: {output:?}"
    );
    let document: Value = serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
            "the relative selector should load the JSON invocation-directory config: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })?;
    ensure!(
        document
            .pointer("/result/content")
            .and_then(Value::as_str)
            .is_some(),
        "JSON output should contain the generated Ninja artefact: {document}",
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Generated selector paths preserve the ADR-014 independence contract.
    #[test]
    fn explicit_config_selection_ignores_directory_anchor(
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
        let result = assert_explicit_config_selection(
            selector,
            selector_path_kind,
            &project_name,
            &config_name,
        );
        prop_assert!(result.is_ok(), "{result:?}");
    }
}
