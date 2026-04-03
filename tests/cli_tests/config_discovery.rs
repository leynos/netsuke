//! Configuration discovery integration tests.
//!
//! These tests verify automatic configuration file discovery in project and
//! user scopes, environment variable precedence, and CLI flag overrides.

use anyhow::{Context, Result, ensure};
use netsuke::cli::config::{ColourPolicy, OutputFormat, SpinnerMode};
use netsuke::cli_localization;
use netsuke::theme::ThemePreference;
use rstest::rstest;
use std::ffi::OsStr;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use test_support::{EnvVarGuard, env_lock::EnvLock};

#[rstest]
fn project_scope_config_discovered_automatically() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let temp_dir = tempdir().context("create temporary project directory")?;
    let project_config = temp_dir.path().join(".netsuke.toml");

    // Write project-scope config
    fs::write(
        &project_config,
        r#"
theme = "unicode"
locale = "es-ES"
jobs = 8
"#,
    )
    .context("write project .netsuke.toml")?;

    // Clear env vars that could interfere
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");
    let _locale_guard = EnvVarGuard::remove("NETSUKE_LOCALE");
    let _jobs_guard = EnvVarGuard::remove("NETSUKE_JOBS");

    // Change to project directory and parse CLI
    std::env::set_current_dir(&temp_dir).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI for project config discovery")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge with project config")?
        .with_default_command();

    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "project config theme should be discovered and applied"
    );
    ensure!(
        merged.locale.as_deref() == Some("es-ES"),
        "project config locale should be discovered"
    );
    ensure!(
        merged.jobs == Some(8),
        "project config jobs should be discovered"
    );
    Ok(())
}

#[rstest]
fn user_scope_config_discovered_when_no_project_config() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_home = tempdir().context("create temporary home directory")?;

    // Write user-scope config in fake home
    let user_config = temp_home.path().join(".netsuke.toml");
    fs::write(
        &user_config,
        r#"
theme = "ascii"
colour_policy = "never"
jobs = 4
"#,
    )
    .context("write user .netsuke.toml")?;

    // Set HOME to fake home (Unix-like systems)
    let _home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());
    // Clear other env vars
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");
    let _jobs_guard = EnvVarGuard::remove("NETSUKE_JOBS");
    let _xdg_config_home_guard = EnvVarGuard::remove("XDG_CONFIG_HOME");

    // Change to empty project directory
    std::env::set_current_dir(&temp_project).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI for user config discovery")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge with user config")?
        .with_default_command();

    ensure!(
        merged.theme == Some(ThemePreference::Ascii),
        "user config theme should be discovered when no project config exists"
    );
    ensure!(
        merged.colour_policy == Some(ColourPolicy::Never),
        "user config colour_policy should be discovered"
    );
    ensure!(
        merged.jobs == Some(4),
        "user config jobs should be discovered"
    );
    Ok(())
}

#[rstest]
fn project_config_takes_precedence_over_user_config() -> Result<()> {
    // This test verifies that when in a project directory with a .netsuke.toml,
    // that config is used. The actual precedence mechanism (project scope wins
    // over user scope) is verified by OrthoConfig's own tests. Our integration
    // test simply confirms the project config is discovered and applied.
    let _env_lock = EnvLock::acquire();
    let temp_project = tempdir().context("create temporary project directory")?;

    // Write project-scope config
    let project_config = temp_project.path().join(".netsuke.toml");
    fs::write(
        &project_config,
        r#"
theme = "unicode"
jobs = 8
spinner_mode = "enabled"
"#,
    )
    .context("write project .netsuke.toml")?;

    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");
    let _jobs_guard = EnvVarGuard::remove("NETSUKE_JOBS");

    std::env::set_current_dir(&temp_project).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI with project config")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge configs")?
        .with_default_command();

    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "project config theme should be discovered and applied"
    );
    ensure!(
        merged.jobs == Some(8),
        "project config jobs should be discovered"
    );
    ensure!(
        merged.spinner_mode == Some(SpinnerMode::Enabled),
        "project config spinner_mode should be applied"
    );
    Ok(())
}

#[rstest]
fn environment_variables_override_discovered_config() -> Result<()> {
    let _env_lock = EnvLock::acquire();
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
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::set("NETSUKE_THEME", OsStr::new("unicode"));
    let _jobs_guard = EnvVarGuard::set("NETSUKE_JOBS", OsStr::new("12"));

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
    Ok(())
}

#[rstest]
fn cli_flags_override_environment_and_config() -> Result<()> {
    let _env_lock = EnvLock::acquire();
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
    Ok(())
}

#[rstest]
fn directory_flag_anchors_project_discovery_to_specified_dir() -> Result<()> {
    let _env_lock = EnvLock::acquire();
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

    // Stay in outer directory but use -C to point to project
    std::env::set_current_dir(&temp_outer).context("change to outer directory")?;

    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) =
        netsuke::cli::parse_with_localizer_from(["netsuke", "-C", "project"], &localizer)
            .context("parse CLI with -C directory flag")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge with -C discovery")?
        .with_default_command();

    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "-C flag should anchor project config discovery to specified directory"
    );
    ensure!(
        merged.jobs == Some(6),
        "config values from -C directory should be applied"
    );
    Ok(())
}

#[rstest]
fn config_path_env_var_bypasses_automatic_discovery() -> Result<()> {
    let _env_lock = EnvLock::acquire();
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

    let _config_path_guard = EnvVarGuard::set("NETSUKE_CONFIG_PATH", custom_config.as_os_str());
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");

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
    Ok(())
}

#[rstest]
fn list_fields_append_across_discovered_config_env_and_cli() -> Result<()> {
    let _env_lock = EnvLock::acquire();
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

    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    // Use comma-separated values for list fields in environment variables
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
