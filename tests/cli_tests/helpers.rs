//! Shared helpers for CLI tests.

use std::ffi::OsString;

pub(super) fn os_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(|arg| OsString::from(*arg)).collect()
}

// ---------------------------------------------------------------------------
// Unix config-environment fixture
// ---------------------------------------------------------------------------

/// Isolated test environment for Unix config-discovery tests.
///
/// Creates empty temporary directories for home and project, acquires
/// `EnvLock`, and sets `HOME`, `XDG_CONFIG_HOME`, and `XDG_CONFIG_DIRS`
/// to empty paths so host-level config files cannot leak into assertions.
pub(super) struct UnixConfigTestEnv {
    pub(super) _env_lock: test_support::env_lock::EnvLock,
    pub(super) temp_home: tempfile::TempDir,
    pub(super) temp_project: tempfile::TempDir,
    _cwd_guard: super::merge::CwdGuard,
    _xdg_home: tempfile::TempDir,
    _home_guard: test_support::EnvVarGuard,
    _xdg_home_guard: test_support::EnvVarGuard,
    _xdg_dirs_guard: test_support::EnvVarGuard,
    _config_path_guard: test_support::EnvVarGuard,
    _config_guard: test_support::EnvVarGuard,
    _diag_json_guard: test_support::EnvVarGuard,
    _output_format_guard: test_support::EnvVarGuard,
}

#[cfg(unix)]
#[rstest::fixture]
pub(super) fn unix_config_env() -> anyhow::Result<UnixConfigTestEnv> {
    use anyhow::Context;
    use std::ffi::OsStr;
    use tempfile::tempdir;
    use test_support::EnvVarGuard;
    use test_support::env_lock::EnvLock;

    let env_lock = EnvLock::acquire();
    let temp_home = tempdir().context("create temporary home directory")?;
    let temp_project = tempdir().context("create temporary project directory")?;
    let cwd_guard =
        super::merge::CwdGuard::acquire().context("capture current working directory")?;
    let xdg_home = tempdir().context("create temporary XDG config home")?;
    let home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());
    let xdg_home_guard = EnvVarGuard::set("XDG_CONFIG_HOME", xdg_home.path().as_os_str());
    let xdg_dirs_guard = EnvVarGuard::set("XDG_CONFIG_DIRS", OsStr::new(""));
    let config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let diag_json_guard = EnvVarGuard::remove("NETSUKE_DIAG_JSON");
    let output_format_guard = EnvVarGuard::remove("NETSUKE_OUTPUT_FORMAT");
    Ok(UnixConfigTestEnv {
        _env_lock: env_lock,
        temp_home,
        temp_project,
        _cwd_guard: cwd_guard,
        _xdg_home: xdg_home,
        _home_guard: home_guard,
        _xdg_home_guard: xdg_home_guard,
        _xdg_dirs_guard: xdg_dirs_guard,
        _config_path_guard: config_path_guard,
        _config_guard: config_guard,
        _diag_json_guard: diag_json_guard,
        _output_format_guard: output_format_guard,
    })
}
