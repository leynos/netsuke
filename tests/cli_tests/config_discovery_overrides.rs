//! Configuration override precedence tests: environment variables over
//! discovered config, CLI flags over both, directory-flag anchoring,
//! explicit config path bypass, and list-field appending across layers.

use super::super::merge_probe::merge_in_child;
use anyhow::{Context, Result, ensure};
use netsuke::cli::config::{ColourPolicy, EmojiPolicy};
use rstest::rstest;
use std::ffi::OsString;
use std::fs;
use tempfile::tempdir;

fn merge_in_project(
    args: &[&str],
    project: &std::path::Path,
    extra_environment: Vec<(OsString, OsString)>,
) -> Result<netsuke::cli::Cli> {
    let mut environment = vec![
        (OsString::from("HOME"), project.as_os_str().to_owned()),
        (
            OsString::from("XDG_CONFIG_HOME"),
            project.join(".config").into_os_string(),
        ),
        (OsString::from("XDG_CONFIG_DIRS"), OsString::new()),
    ];
    environment.extend(extra_environment);
    merge_in_child(args, project, &environment)
}

#[rstest]
fn environment_variables_override_discovered_config() -> Result<()> {
    let temp_project = tempdir().context("create temporary project directory")?;

    // Write project-scope config
    let project_config = temp_project.path().join(".netsuke.toml");
    fs::write(
        &project_config,
        r#"
emoji = "never"
jobs = 4
json = false
"#,
    )
    .context("write project .netsuke.toml")?;

    let merged = merge_in_project(
        &["netsuke"],
        temp_project.path(),
        vec![
            (OsString::from("NETSUKE_EMOJI"), OsString::from("always")),
            (OsString::from("NETSUKE_JOBS"), OsString::from("12")),
        ],
    )?;

    ensure!(
        merged.emoji == EmojiPolicy::Always,
        "environment variable should override project config emoji policy"
    );
    ensure!(
        merged.jobs == Some(12),
        "environment variable should override project config jobs"
    );
    ensure!(
        !merged.json,
        "project config JSON value should apply when no env override exists"
    );
    // Restore the cwd before `temp_project` (declared later) drops: implicit
    // reverse-declaration drop order would remove the temp dir while it is still
    // the process cwd, which fails on Windows.
    Ok(())
}

#[rstest]
fn cli_flags_override_environment_and_config() -> Result<()> {
    let temp_project = tempdir().context("create temporary project directory")?;

    // Write project-scope config
    let project_config = temp_project.path().join(".netsuke.toml");
    fs::write(
        &project_config,
        r#"
emoji = "never"
jobs = 4
color = "never"
json = false
"#,
    )
    .context("write project .netsuke.toml")?;

    let merged = merge_in_project(
        &["netsuke", "--emoji", "never", "--jobs", "16", "--json"],
        temp_project.path(),
        vec![
            (OsString::from("NETSUKE_EMOJI"), OsString::from("always")),
            (OsString::from("NETSUKE_JOBS"), OsString::from("8")),
            (OsString::from("NETSUKE_COLOR"), OsString::from("always")),
        ],
    )?;

    ensure!(
        merged.emoji == EmojiPolicy::Never,
        "CLI emoji flag should override environment and config"
    );
    ensure!(
        merged.jobs == Some(16),
        "CLI jobs flag should override environment and config"
    );
    ensure!(merged.json, "CLI JSON flag should override config");
    ensure!(
        merged.color == ColourPolicy::Always,
        "environment color policy should apply when CLI does not override"
    );
    Ok(())
}

#[rstest]
#[case("-C")]
#[case("--directory")]
fn directory_flag_anchors_project_discovery_to_specified_dir(#[case] flag: &str) -> Result<()> {
    let temp_outer = tempdir().context("create outer directory")?;
    let temp_project = temp_outer.path().join("project");
    fs::create_dir(&temp_project).context("create project subdirectory")?;

    // Write config in the specified project directory
    let project_config = temp_project.join(".netsuke.toml");
    fs::write(
        &project_config,
        r#"
emoji = "always"
jobs = 6
"#,
    )
    .context("write project .netsuke.toml in subdirectory")?;

    // Stay in outer directory but use directory flag to point to project
    let merged = merge_in_project(&["netsuke", flag, "project"], temp_outer.path(), Vec::new())?;

    ensure!(
        merged.emoji == EmojiPolicy::Always,
        "directory flag should anchor project config discovery to specified directory"
    );
    ensure!(
        merged.jobs == Some(6),
        "config values from directory flag should be applied"
    );
    Ok(())
}

#[rstest]
fn config_path_env_var_bypasses_automatic_discovery() -> Result<()> {
    let temp_project = tempdir().context("create project directory")?;
    let temp_custom = tempdir().context("create custom config directory")?;

    // Write project-scope config (should be ignored)
    let project_config = temp_project.path().join(".netsuke.toml");
    fs::write(
        &project_config,
        r#"
emoji = "never"
jobs = 2
"#,
    )
    .context("write project .netsuke.toml")?;

    // Write custom config that should be used via NETSUKE_CONFIG.
    let custom_config = temp_custom.path().join("custom.toml");
    fs::write(
        &custom_config,
        r#"
emoji = "always"
jobs = 16
color = "always"
"#,
    )
    .context("write custom config")?;

    let merged = merge_in_project(
        &["netsuke"],
        temp_project.path(),
        vec![(
            OsString::from("NETSUKE_CONFIG"),
            custom_config.into_os_string(),
        )],
    )?;

    ensure!(
        merged.emoji == EmojiPolicy::Always,
        "NETSUKE_CONFIG should bypass automatic discovery"
    );
    ensure!(
        merged.jobs == Some(16),
        "custom config jobs should be used instead of project config"
    );
    ensure!(
        merged.color == ColourPolicy::Always,
        "custom config color policy should be applied"
    );
    Ok(())
}

/// Assert that `default_targets` and `fetch_allow_scheme` have been appended
/// in config → env → CLI order by the merge pipeline.
fn assert_list_fields_appended(merged: &netsuke::cli::Cli) -> Result<()> {
    // Verify layer order for default_targets: config ["fmt", "lint"] -> env ["test"] -> CLI ["build"]
    ensure!(
        merged
            .default_targets
            .starts_with(&["fmt".to_owned(), "lint".to_owned()]),
        "default_targets should start with config layer entries [\"fmt\", \"lint\"]"
    );
    ensure!(
        merged.default_targets.len() >= 3
            && merged.default_targets.get(2) == Some(&"test".to_owned()),
        "default_targets should have env layer entry \"test\" after config entries"
    );
    ensure!(
        merged.default_targets.len() >= 4
            && merged.default_targets.get(3) == Some(&"build".to_owned()),
        "default_targets should have CLI layer entry \"build\" after env entry"
    );

    // Verify layer order for fetch_allow_scheme: config ["https"] -> env ["http"] -> CLI ["ftp"]
    ensure!(
        merged.fetch_allow_scheme.starts_with(&["https".to_owned()]),
        "fetch_allow_scheme should start with config layer entry [\"https\"]"
    );
    ensure!(
        merged.fetch_allow_scheme.len() >= 2
            && merged.fetch_allow_scheme.get(1) == Some(&"http".to_owned()),
        "fetch_allow_scheme should have env layer entry \"http\" after config entry"
    );
    ensure!(
        merged.fetch_allow_scheme.len() >= 3
            && merged.fetch_allow_scheme.get(2) == Some(&"ftp".to_owned()),
        "fetch_allow_scheme should have CLI layer entry \"ftp\" after env entry"
    );

    // Final full-vector equality checks
    ensure!(
        merged.default_targets == vec!["fmt", "lint", "test", "build"],
        "default_targets should append across config, env, and CLI layers"
    );
    ensure!(
        merged.fetch_allow_scheme == vec!["https", "http", "ftp"],
        "fetch_allow_scheme should append across layers"
    );
    Ok(())
}

#[rstest]
fn list_fields_append_across_discovered_config_env_and_cli() -> Result<()> {
    let temp_project = tempdir().context("create project directory")?;

    // Write project config with default_targets
    let project_config = temp_project.path().join(".netsuke.toml");
    fs::write(
        &project_config,
        r#"
default_targets = ["fmt", "lint"]
fetch_allow_scheme = ["https"]
"#,
    )
    .context("write project .netsuke.toml with lists")?;

    let merged = merge_in_project(
        &[
            "netsuke",
            "--default-target",
            "build",
            "--fetch-allow-scheme",
            "ftp",
        ],
        temp_project.path(),
        vec![
            (
                OsString::from("NETSUKE_DEFAULT_TARGETS"),
                OsString::from("test"),
            ),
            (
                OsString::from("NETSUKE_FETCH_ALLOW_SCHEME"),
                OsString::from("http"),
            ),
        ],
    )?;

    assert_list_fields_appended(&merged)
}
