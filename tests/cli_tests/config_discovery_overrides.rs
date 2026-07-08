//! Configuration override precedence tests: environment variables over
//! discovered config, CLI flags over both, directory-flag anchoring,
//! explicit config path bypass, and list-field appending across layers.

use anyhow::{Context, Result, ensure};
use netsuke::cli::config::{ColourPolicy, OutputFormat};
use netsuke::cli_localization;
use netsuke::theme::ThemePreference;
use rstest::rstest;
use std::ffi::OsStr;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use test_support::{EnvVarGuard, env_lock::EnvLock};

use super::CwdGuard;

#[rstest]
fn environment_variables_override_discovered_config() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;
    let temp_project = tempdir().context("create temporary project directory")?;

    // Write project-scope config
    let project_config = temp_project.path().join(".netsuke.toml");
    fs::write(
        &project_config,
        r#"
theme = "ascii"
jobs = 4
output_format = "human"
"#,
    )
    .context("write project .netsuke.toml")?;

    // Set environment variables that should override the file
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::set("NETSUKE_THEME", OsStr::new("unicode"));
    let _jobs_guard = EnvVarGuard::set("NETSUKE_JOBS", OsStr::new("12"));
    let _output_format_guard = EnvVarGuard::remove("NETSUKE_OUTPUT_FORMAT");
    let _colour_guard = EnvVarGuard::remove("NETSUKE_COLOUR_POLICY");

    std::env::set_current_dir(&temp_project).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) =
        netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer).context("parse CLI")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge with env overrides")?
        .with_default_command();

    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "environment variable should override project config theme"
    );
    ensure!(
        merged.jobs == Some(12),
        "environment variable should override project config jobs"
    );
    ensure!(
        merged.output_format == Some(OutputFormat::Human),
        "project config value should apply when no env override exists"
    );
    drop(cwd_guard);
    Ok(())
}

#[rstest]
fn cli_flags_override_environment_and_config() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;
    let temp_project = tempdir().context("create temporary project directory")?;

    // Write project-scope config
    let project_config = temp_project.path().join(".netsuke.toml");
    fs::write(
        &project_config,
        r#"
theme = "ascii"
jobs = 4
colour_policy = "never"
output_format = "human"
"#,
    )
    .context("write project .netsuke.toml")?;

    // Set environment variables
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::set("NETSUKE_THEME", OsStr::new("unicode"));
    let _jobs_guard = EnvVarGuard::set("NETSUKE_JOBS", OsStr::new("8"));
    let _colour_guard = EnvVarGuard::set("NETSUKE_COLOUR_POLICY", OsStr::new("always"));

    std::env::set_current_dir(&temp_project).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    // CLI flags should win
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(
        [
            "netsuke",
            "--theme",
            "ascii",
            "--jobs",
            "16",
            "--output-format",
            "json",
        ],
        &localizer,
    )
    .context("parse CLI with flag overrides")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge with CLI overrides")?
        .with_default_command();

    ensure!(
        merged.theme == Some(ThemePreference::Ascii),
        "CLI theme flag should override environment and config"
    );
    ensure!(
        merged.jobs == Some(16),
        "CLI jobs flag should override environment and config"
    );
    ensure!(
        merged.output_format == Some(OutputFormat::Json),
        "CLI output_format flag should override config"
    );
    ensure!(
        merged.colour_policy == Some(ColourPolicy::Always),
        "environment colour_policy should apply when CLI does not override"
    );
    drop(cwd_guard);
    Ok(())
}

#[rstest]
#[case("-C")]
#[case("--directory")]
fn directory_flag_anchors_project_discovery_to_specified_dir(#[case] flag: &str) -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;
    let temp_outer = tempdir().context("create outer directory")?;
    let temp_project = temp_outer.path().join("project");
    fs::create_dir(&temp_project).context("create project subdirectory")?;

    // Write config in the specified project directory
    let project_config = temp_project.join(".netsuke.toml");
    fs::write(
        &project_config,
        r#"
theme = "unicode"
jobs = 6
"#,
    )
    .context("write project .netsuke.toml in subdirectory")?;

    // Stay in outer directory but use directory flag to point to project
    std::env::set_current_dir(&temp_outer).context("change to outer directory")?;

    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");
    let _jobs_guard = EnvVarGuard::remove("NETSUKE_JOBS");
    let _output_format_guard = EnvVarGuard::remove("NETSUKE_OUTPUT_FORMAT");
    let _colour_guard = EnvVarGuard::remove("NETSUKE_COLOUR_POLICY");

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) =
        netsuke::cli::parse_with_localizer_from(["netsuke", flag, "project"], &localizer)
            .context("parse CLI with directory flag")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge with directory flag discovery")?
        .with_default_command();

    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "directory flag should anchor project config discovery to specified directory"
    );
    ensure!(
        merged.jobs == Some(6),
        "config values from directory flag should be applied"
    );
    drop(cwd_guard);
    Ok(())
}

#[rstest]
fn config_path_env_var_bypasses_automatic_discovery() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;
    let temp_project = tempdir().context("create project directory")?;
    let temp_custom = tempdir().context("create custom config directory")?;

    // Write project-scope config (should be ignored)
    let project_config = temp_project.path().join(".netsuke.toml");
    fs::write(
        &project_config,
        r#"
theme = "ascii"
jobs = 2
"#,
    )
    .context("write project .netsuke.toml")?;

    // Write custom config that should be used via NETSUKE_CONFIG_PATH
    let custom_config = temp_custom.path().join("custom.toml");
    fs::write(
        &custom_config,
        r#"
theme = "unicode"
jobs = 16
colour_policy = "always"
"#,
    )
    .context("write custom config")?;

    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _config_path_guard = EnvVarGuard::set("NETSUKE_CONFIG_PATH", custom_config.as_os_str());
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");
    let _jobs_guard = EnvVarGuard::remove("NETSUKE_JOBS");
    let _output_format_guard = EnvVarGuard::remove("NETSUKE_OUTPUT_FORMAT");
    let _colour_guard = EnvVarGuard::remove("NETSUKE_COLOUR_POLICY");

    std::env::set_current_dir(&temp_project).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI with NETSUKE_CONFIG_PATH")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge with explicit config path")?
        .with_default_command();

    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "NETSUKE_CONFIG_PATH should bypass automatic discovery"
    );
    ensure!(
        merged.jobs == Some(16),
        "custom config jobs should be used instead of project config"
    );
    ensure!(
        merged.colour_policy == Some(ColourPolicy::Always),
        "custom config colour_policy should be applied"
    );
    drop(cwd_guard);
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
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;
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

    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    // Set single-value environment variables for list fields
    let _targets_guard = EnvVarGuard::set("NETSUKE_DEFAULT_TARGETS", OsStr::new("test"));
    let _scheme_guard = EnvVarGuard::set("NETSUKE_FETCH_ALLOW_SCHEME", OsStr::new("http"));

    std::env::set_current_dir(&temp_project).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(
        [
            "netsuke",
            "--default-target",
            "build",
            "--fetch-allow-scheme",
            "ftp",
        ],
        &localizer,
    )
    .context("parse CLI with list overrides")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge with list appending")?
        .with_default_command();

    let result = assert_list_fields_appended(&merged);
    drop(cwd_guard);
    result
}
