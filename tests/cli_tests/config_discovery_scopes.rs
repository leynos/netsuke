//! Project- and user-scope configuration discovery tests: automatic
//! project-file discovery, user-scope fallback, and project-over-user
//! precedence on Unix and Windows.

#[cfg(unix)]
use super::super::merge_probe::environment_with_system_scope;
use super::super::merge_probe::{isolated_environment, merge_in_child};
use anyhow::{Context, Result, ensure};
use netsuke::cli::config::{ColourPolicy, EmojiPolicy};
use rstest::rstest;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn run_scope_scenario(
    project: &Path,
    home: &Path,
    overrides: &[(OsString, OsString)],
) -> Result<netsuke::cli::Cli> {
    let (_xdg_config_dirs, environment) = isolated_environment(home, overrides)?;
    merge_in_child(&["netsuke"], project, &environment)
}

#[rstest]
fn project_scope_config_discovered_automatically() -> Result<()> {
    let temp_dir = tempdir().context("create temporary project directory")?;
    let project_config = temp_dir.path().join(".netsuke.toml");

    // Write project-scope config
    fs::write(
        &project_config,
        r#"
emoji = "always"
locale = "es-ES"
jobs = 8
"#,
    )
    .context("write project .netsuke.toml")?;

    // Keep home separate from the project directory so the assertions below can
    // only pass through project-scope discovery.
    let home_dir = tempdir().context("create temporary home directory")?;
    let merged = run_scope_scenario(temp_dir.path(), home_dir.path(), &[])?;

    ensure!(
        merged.emoji == EmojiPolicy::Always,
        "project config emoji policy should be discovered and applied"
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

/// User-scope config content shared by the Unix and Windows test variants.
const USER_CONFIG_CONTENT: &str = r#"
emoji = "never"
color = "never"
jobs = 4
"#;

fn assert_user_config_applied(merged: &netsuke::cli::Cli) -> Result<()> {
    ensure!(
        merged.emoji == EmojiPolicy::Never,
        "user config emoji policy should be discovered when no project config exists"
    );
    ensure!(
        merged.color == ColourPolicy::Never,
        "user config color policy should be discovered"
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
    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_home = tempdir().context("create temporary home directory")?;

    // Write user-scope config in fake home
    fs::write(temp_home.path().join(".netsuke.toml"), USER_CONFIG_CONTENT)
        .context("write user .netsuke.toml")?;

    // Sandbox XDG paths so system-wide configs cannot leak into the test
    let xdg_config_home = temp_home.path().join(".config");
    fs::create_dir_all(&xdg_config_home).context("create sandboxed XDG_CONFIG_HOME")?;
    let merged = run_scope_scenario(
        temp_project.path(),
        temp_home.path(),
        &[(
            OsString::from("XDG_CONFIG_HOME"),
            xdg_config_home.into_os_string(),
        )],
    )?;
    assert_user_config_applied(&merged)
}

#[cfg(windows)]
#[rstest]
fn user_scope_config_discovered_when_no_project_config() -> Result<()> {
    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_appdata = tempdir().context("create temporary APPDATA directory")?;

    // Create netsuke subdirectory in fake APPDATA
    let netsuke_config_dir = temp_appdata.path().join("netsuke");
    fs::create_dir_all(&netsuke_config_dir).context("create netsuke config directory")?;

    // Write user-scope config in fake APPDATA
    fs::write(netsuke_config_dir.join("config.toml"), USER_CONFIG_CONTENT)
        .context("write user config.toml in APPDATA")?;

    // Set APPDATA to fake directory (Windows)
    let merged = run_scope_scenario(
        temp_project.path(),
        temp_project.path(),
        &[(
            OsString::from("APPDATA"),
            temp_appdata.path().as_os_str().to_owned(),
        )],
    )?;
    assert_user_config_applied(&merged)
}

/// Project config TOML used by both Unix and Windows precedence test variants.
const PRECEDENCE_PROJECT_CONFIG_CONTENT: &str = r#"
emoji = "always"
jobs = 8
"#;

/// User config TOML used by both Unix and Windows precedence test variants.
const PRECEDENCE_USER_CONFIG_CONTENT: &str = r#"
emoji = "never"
color = "never"
"#;

fn assert_project_precedence_applied(merged: &netsuke::cli::Cli) -> Result<()> {
    ensure!(
        merged.emoji == EmojiPolicy::Always,
        "project config emoji policy should override user config"
    );
    ensure!(
        merged.jobs == Some(8),
        "project config jobs should be applied"
    );
    ensure!(
        merged.color == ColourPolicy::Never,
        "user-only field should still be merged when project config does not override it"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn project_config_takes_precedence_over_user_config() -> Result<()> {
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
    let merged = run_scope_scenario(
        temp_project.path(),
        temp_home.path(),
        &[(
            OsString::from("XDG_CONFIG_HOME"),
            temp_xdg_home.path().as_os_str().to_owned(),
        )],
    )?;
    assert_project_precedence_applied(&merged)
}

#[cfg(windows)]
#[rstest]
fn project_config_takes_precedence_over_user_config() -> Result<()> {
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

    let merged = run_scope_scenario(
        temp_project.path(),
        temp_project.path(),
        &[(
            OsString::from("APPDATA"),
            temp_appdata.path().as_os_str().to_owned(),
        )],
    )?;
    assert_project_precedence_applied(&merged)
}

/// System-scope config content used by the Unix variants. Windows discovers
/// user and system configuration through `APPDATA`/`LOCALAPPDATA` rather than
/// the XDG variables these tests inject, so the scenarios below are Unix-only.
#[cfg(unix)]
const SYSTEM_CONFIG_CONTENT: &str = r#"
file = "Systemfile"
emoji = "always"
color = "always"
jobs = 9
locale = "de-DE"
"#;

#[cfg(unix)]
fn assert_system_config_applied(merged: &netsuke::cli::Cli) -> Result<()> {
    ensure!(
        merged.file.as_path() == Path::new("Systemfile"),
        "system config manifest path should be discovered when no user or project config exists"
    );
    ensure!(
        merged.emoji == EmojiPolicy::Always,
        "system config emoji policy should be discovered when no user or project config exists"
    );
    ensure!(
        merged.color == ColourPolicy::Always,
        "system config color policy should be discovered"
    );
    ensure!(
        merged.jobs == Some(9),
        "system config jobs should be discovered"
    );
    ensure!(
        merged.locale.as_deref() == Some("de-DE"),
        "system config locale should be discovered"
    );
    Ok(())
}

/// Write discovery-scope config files and merge in an isolated child rooted at
/// `project`. The selector set stays closed at the documented variables; the
/// `system` `TempDir` is discarded after the child exits. Unix-only because the
/// injected environment uses the XDG variables that Windows does not read.
#[cfg(unix)]
fn run_system_scope_scenario(
    project: &Path,
    home: &Path,
    system: &Path,
    scopes: &ScopeLayers,
) -> Result<netsuke::cli::Cli> {
    let system_dir = system.join("netsuke");
    fs::create_dir_all(&system_dir)
        .with_context(|| format!("create system config directory {}", system_dir.display()))?;
    fs::write(system_dir.join("config.toml"), SYSTEM_CONFIG_CONTENT)
        .context("write system config")?;
    if let Some(user_content) = scopes.user_config {
        let user_dir = home.join(".config").join("netsuke");
        fs::create_dir_all(&user_dir).context("create user config directory")?;
        fs::write(user_dir.join("config.toml"), user_content).context("write user config")?;
    }
    if let Some(project_content) = scopes.project_config {
        fs::write(project.join(".netsuke.toml"), project_content)
            .context("write project config")?;
    }
    let environment = environment_with_system_scope(home, system, &[]);
    merge_in_child(&["netsuke"], project, &environment)
}

/// Optional user- and project-scope layers for a system-scope discovery run.
#[cfg(unix)]
#[derive(Default)]
struct ScopeLayers {
    user_config: Option<&'static str>,
    project_config: Option<&'static str>,
}

/// System-scope discovery is platform-neutral here: `run_system_scope_scenario`
/// points the child at an isolated `XDG_CONFIG_DIRS` via the injected
/// environment seam. The XDG variables are not read on Windows (which uses
/// `APPDATA`/`LOCALAPPDATA`), so the discovery scenario is Unix-only.
#[cfg(unix)]
#[rstest]
fn system_scope_config_discovered_when_no_user_or_project_config() -> Result<()> {
    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_home = tempdir().context("create temporary home directory")?;
    let temp_system = tempdir().context("create temporary system directory")?;
    let merged = run_system_scope_scenario(
        temp_project.path(),
        temp_home.path(),
        temp_system.path(),
        &ScopeLayers::default(),
    )?;
    assert_system_config_applied(&merged)
}

#[cfg(unix)]
#[rstest]
fn user_scope_config_takes_precedence_over_system_scope() -> Result<()> {
    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_home = tempdir().context("create temporary home directory")?;
    let temp_system = tempdir().context("create temporary system directory")?;
    let merged = run_system_scope_scenario(
        temp_project.path(),
        temp_home.path(),
        temp_system.path(),
        &ScopeLayers {
            user_config: Some("emoji = \"never\"\njobs = 4\n"),
            ..ScopeLayers::default()
        },
    )?;
    // OrthoConfig discovery is exclusive: the user layer wins over the system
    // layer for every overlapping field, and the system file is not merged.
    ensure!(
        merged.file.as_path() == Path::new("Netsukefile"),
        "manifest path should fall back to the default when the system layer loses, got {:?}",
        merged.file
    );
    ensure!(
        merged.emoji == EmojiPolicy::Never,
        "user config emoji policy should override system config"
    );
    ensure!(
        merged.jobs == Some(4),
        "user config jobs should override system config"
    );
    ensure!(
        merged.color == ColourPolicy::Auto,
        "system color field should not appear when the user layer wins"
    );
    Ok(())
}

/// Project-scope config coexists with a system-scope file: the project layer
/// outranks the system layer for overlapping fields, and a system-only field
/// still merges through.
#[cfg(unix)]
#[rstest]
fn project_config_overrides_system_and_system_only_field_merges() -> Result<()> {
    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_home = tempdir().context("create temporary home directory")?;
    let temp_system = tempdir().context("create temporary system directory")?;
    let merged = run_system_scope_scenario(
        temp_project.path(),
        temp_home.path(),
        temp_system.path(),
        &ScopeLayers {
            project_config: Some("emoji = \"never\"\njobs = 4\n"),
            ..ScopeLayers::default()
        },
    )?;
    ensure!(
        merged.file.as_path() == Path::new("Systemfile"),
        "system-only manifest path should merge through when project config does not set it"
    );
    ensure!(
        merged.emoji == EmojiPolicy::Never,
        "project config emoji policy should override system config"
    );
    ensure!(
        merged.jobs == Some(4),
        "project config jobs should override system config"
    );
    ensure!(
        merged.color == ColourPolicy::Always,
        "system-only color field should merge through when project config does not set it"
    );
    ensure!(
        merged.locale.as_deref() == Some("de-DE"),
        "system-only locale should merge through when project config does not set it"
    );
    Ok(())
}
