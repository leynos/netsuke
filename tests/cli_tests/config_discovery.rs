//! Configuration discovery integration tests.
//!
//! These tests verify automatic configuration file discovery in project and
//! user scopes, environment variable precedence, and CLI flag overrides.

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

/// RAII guard that restores the process working directory on drop.
///
/// Acquire this *after* `EnvLock` so the drop order (CWD restored first,
/// lock released second) mirrors the acquire order.
struct CwdGuard(std::path::PathBuf);

impl CwdGuard {
    fn acquire() -> anyhow::Result<Self> {
        Ok(Self(
            std::env::current_dir().context("capture current working directory")?,
        ))
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        drop(std::env::set_current_dir(&self.0));
    }
}

#[rstest]
fn project_scope_config_discovered_automatically() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;
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
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
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
    drop(cwd_guard);
    Ok(())
}

/// User-scope config content shared by the Unix and Windows test variants.
const USER_CONFIG_CONTENT: &str = r#"
theme = "ascii"
colour_policy = "never"
jobs = 4
"#;

fn assert_user_config_applied(merged: &netsuke::cli::Cli) -> Result<()> {
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

#[cfg(unix)]
#[rstest]
fn user_scope_config_discovered_when_no_project_config() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;
    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_home = tempdir().context("create temporary home directory")?;

    // Write user-scope config in fake home
    fs::write(temp_home.path().join(".netsuke.toml"), USER_CONFIG_CONTENT)
        .context("write user .netsuke.toml")?;

    // Set HOME to fake home (Unix-like systems)
    let _home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());
    // Sandbox XDG paths so system-wide configs cannot leak into the test
    let xdg_config_home = temp_home.path().join(".config");
    fs::create_dir_all(&xdg_config_home).context("create sandboxed XDG_CONFIG_HOME")?;
    let _xdg_config_home_guard = EnvVarGuard::set("XDG_CONFIG_HOME", xdg_config_home.as_os_str());
    let _xdg_config_dirs_guard = EnvVarGuard::set("XDG_CONFIG_DIRS", OsStr::new(""));
    // Clear other env vars
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");
    let _jobs_guard = EnvVarGuard::remove("NETSUKE_JOBS");
    let _colour_policy_guard = EnvVarGuard::remove("NETSUKE_COLOUR_POLICY");

    // Change to empty project directory
    std::env::set_current_dir(&temp_project).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI for user config discovery")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge with user config")?
        .with_default_command();

    let result = assert_user_config_applied(&merged);
    drop(cwd_guard);
    result
}

#[cfg(windows)]
#[rstest]
fn user_scope_config_discovered_when_no_project_config() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;
    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_appdata = tempdir().context("create temporary APPDATA directory")?;

    // Create netsuke subdirectory in fake APPDATA
    let netsuke_config_dir = temp_appdata.path().join("netsuke");
    fs::create_dir_all(&netsuke_config_dir).context("create netsuke config directory")?;

    // Write user-scope config in fake APPDATA
    fs::write(netsuke_config_dir.join("config.toml"), USER_CONFIG_CONTENT)
        .context("write user config.toml in APPDATA")?;

    // Set APPDATA to fake directory (Windows)
    let _appdata_guard = EnvVarGuard::set("APPDATA", temp_appdata.path().as_os_str());
    // Clear other env vars
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");
    let _jobs_guard = EnvVarGuard::remove("NETSUKE_JOBS");
    let _colour_policy_guard = EnvVarGuard::remove("NETSUKE_COLOUR_POLICY");
    let _localappdata_guard = EnvVarGuard::remove("LOCALAPPDATA");

    // Change to empty project directory
    std::env::set_current_dir(&temp_project).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI for user config discovery")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge with user config")?
        .with_default_command();

    let result = assert_user_config_applied(&merged);
    drop(cwd_guard);
    result
}

/// Project config TOML used by both Unix and Windows precedence test variants.
const PRECEDENCE_PROJECT_CONFIG_CONTENT: &str = r#"
theme = "unicode"
jobs = 8
"#;

/// User config TOML used by both Unix and Windows precedence test variants.
const PRECEDENCE_USER_CONFIG_CONTENT: &str = r#"
theme = "ascii"
colour_policy = "never"
"#;

fn assert_project_precedence_applied(merged: &netsuke::cli::Cli) -> Result<()> {
    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "project config theme should override user config theme"
    );
    ensure!(
        merged.jobs == Some(8),
        "project config jobs should be applied"
    );
    ensure!(
        merged.colour_policy == Some(ColourPolicy::Never),
        "user-only field should still be merged when project config does not override it"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn project_config_takes_precedence_over_user_config() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;

    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_home = tempdir().context("create temporary home directory")?;

    // User config: sets theme and a user-only field (colour_policy).
    fs::write(
        temp_home.path().join(".netsuke.toml"),
        PRECEDENCE_USER_CONFIG_CONTENT,
    )
    .context("write user .netsuke.toml")?;

    // Project config: overrides theme; does NOT set colour_policy.
    fs::write(
        temp_project.path().join(".netsuke.toml"),
        PRECEDENCE_PROJECT_CONFIG_CONTENT,
    )
    .context("write project .netsuke.toml")?;

    let temp_xdg_home = tempdir().context("create temporary XDG config home")?;
    let _home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());
    let _xdg_home_guard = EnvVarGuard::set("XDG_CONFIG_HOME", temp_xdg_home.path().as_os_str());
    let _xdg_dirs_guard = EnvVarGuard::set("XDG_CONFIG_DIRS", OsStr::new(""));
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");
    let _jobs_guard = EnvVarGuard::remove("NETSUKE_JOBS");
    let _colour_guard = EnvVarGuard::remove("NETSUKE_COLOUR_POLICY");

    std::env::set_current_dir(&temp_project).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI for precedence test")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge configs")?
        .with_default_command();

    let result = assert_project_precedence_applied(&merged);
    drop(cwd_guard);
    result
}

#[cfg(windows)]
#[rstest]
fn project_config_takes_precedence_over_user_config() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;

    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_appdata = tempdir().context("create temporary APPDATA directory")?;

    // Create sandboxed Windows user-scope config at %APPDATA%\netsuke\config.toml
    let netsuke_config_dir = temp_appdata.path().join("netsuke");
    fs::create_dir_all(&netsuke_config_dir).context("create netsuke config directory")?;
    fs::write(
        netsuke_config_dir.join("config.toml"),
        PRECEDENCE_USER_CONFIG_CONTENT,
    )
    .context("write user config.toml in APPDATA")?;

    // Project config: overrides theme; does NOT set colour_policy.
    fs::write(
        temp_project.path().join(".netsuke.toml"),
        PRECEDENCE_PROJECT_CONFIG_CONTENT,
    )
    .context("write project .netsuke.toml")?;

    let _appdata_guard = EnvVarGuard::set("APPDATA", temp_appdata.path().as_os_str());
    let _localappdata_guard = EnvVarGuard::remove("LOCALAPPDATA");
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");
    let _jobs_guard = EnvVarGuard::remove("NETSUKE_JOBS");
    let _colour_guard = EnvVarGuard::remove("NETSUKE_COLOUR_POLICY");

    std::env::set_current_dir(&temp_project).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI for precedence test")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge configs")?
        .with_default_command();

    let result = assert_project_precedence_applied(&merged);
    drop(cwd_guard);
    result
}

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
