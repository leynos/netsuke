//! Unit tests for private helpers in `config_merge`.

use super::*;
use anyhow::ensure;
use clap::CommandFactory;
use rstest::{fixture, rstest};
use serde_json::json;
use tempfile::tempdir;
use test_support::{CwdGuard, EnvVarGuard};

fn cli_with_directory(directory: &std::path::Path) -> Cli {
    Cli {
        directory: Some(directory.to_path_buf()),
        ..Cli::default()
    }
}

// ---------------------------------------------------------------------------
// is_empty_value
// ---------------------------------------------------------------------------

#[test]
fn is_empty_value_accepts_empty_object() {
    assert!(is_empty_value(&json!({})));
}

#[rstest]
#[case::string(json!("hello"))]
#[case::number(json!(42))]
#[case::null(json!(null))]
#[case::boolean(json!(true))]
#[case::array(json!([]))]
fn is_empty_value_rejects_non_object_types(#[case] value: serde_json::Value) {
    assert!(!is_empty_value(&value));
}

#[test]
fn is_empty_value_rejects_populated_object() {
    assert!(!is_empty_value(&json!({"theme": "ascii"})));
}

// ---------------------------------------------------------------------------
// diag_json_from_layer
// ---------------------------------------------------------------------------

#[test]
fn diag_json_from_layer_returns_none_for_non_object() {
    assert_eq!(diag_json_from_layer(&json!("hello")), None);
}

#[test]
fn diag_json_from_layer_returns_none_when_neither_field_present() {
    assert_eq!(diag_json_from_layer(&json!({"theme": "ascii"})), None);
}

#[rstest]
#[case::true_value(json!({"diag_json": true}), Some(true))]
#[case::false_value(json!({"diag_json": false}), Some(false))]
fn diag_json_from_layer_reads_diag_json_bool(
    #[case] layer: serde_json::Value,
    #[case] expected: Option<bool>,
) {
    assert_eq!(diag_json_from_layer(&layer), expected);
}

#[test]
fn diag_json_from_layer_returns_none_for_non_bool_diag_json() {
    assert_eq!(diag_json_from_layer(&json!({"diag_json": "yes"})), None);
}

#[rstest]
#[case::json_format(json!({"output_format": "json"}), Some(true))]
#[case::human_format(json!({"output_format": "human"}), Some(false))]
fn diag_json_from_layer_prefers_output_format_over_diag_json(
    #[case] layer: serde_json::Value,
    #[case] expected: Option<bool>,
) {
    assert_eq!(diag_json_from_layer(&layer), expected);
}

#[test]
fn diag_json_from_layer_output_format_wins_over_diag_json() {
    let layer = json!({"output_format": "human", "diag_json": true});
    assert_eq!(
        diag_json_from_layer(&layer),
        Some(false),
        "output_format should take precedence over diag_json"
    );
}

#[test]
fn diag_json_from_layer_ignores_invalid_output_format() {
    let layer = json!({"output_format": "tap", "diag_json": true});
    assert_eq!(
        diag_json_from_layer(&layer),
        Some(true),
        "invalid output_format should fall through to diag_json"
    );
}

// ---------------------------------------------------------------------------
// project_scope_file_str
// ---------------------------------------------------------------------------

#[rstest]
#[case::file_absent(false)]
#[case::file_present(true)]
fn project_scope_file_str_returns_expected_path(#[case] create_file: bool) {
    let dir = tempdir().expect("tempdir");
    if create_file {
        std::fs::write(dir.path().join(".netsuke.toml"), "").expect("write");
    }
    let expected_path = dir.path().join(".netsuke.toml");
    let expected = expected_path.to_string_lossy().into_owned();
    let path = project_scope_file_str(Some(dir.path())).expect("should have a path");
    assert_eq!(
        path, expected,
        "returned path should be anchored to the dir"
    );
}

#[test]
fn project_scope_file_str_uses_cwd_when_directory_is_none() {
    use test_support::env_lock::EnvLock;
    let _lock = EnvLock::acquire();
    let _cwd_guard = CwdGuard::new().expect("capture cwd");
    // When directory is None the helper falls back to cwd
    let dir = tempdir().expect("tempdir");
    std::env::set_current_dir(&dir).expect("chdir");
    let expected_path = dir.path().join(".netsuke.toml");
    let expected = expected_path.to_string_lossy().into_owned();
    let path = project_scope_file_str(None).expect("should have a path");
    assert_eq!(path, expected, "returned path should be anchored to cwd");
}

// ---------------------------------------------------------------------------
// project_scope_layers
// ---------------------------------------------------------------------------

#[test]
fn project_scope_layers_returns_empty_when_no_file_present() {
    let dir = tempdir().expect("tempdir");
    let layers = project_scope_layers(Some(dir.path())).expect("should succeed");
    assert!(
        layers.is_empty(),
        "no layers expected when no config file is present"
    );
}

#[test]
fn project_scope_layers_returns_one_layer_when_file_present() {
    let dir = tempdir().expect("tempdir");
    std::fs::write(dir.path().join(".netsuke.toml"), r#"theme = "ascii""#).expect("write config");
    let layers = project_scope_layers(Some(dir.path())).expect("should succeed");
    assert_eq!(layers.len(), 1, "exactly one layer expected");
}

// ---------------------------------------------------------------------------
// collect_diag_file_layers
// ---------------------------------------------------------------------------

/// Fixture that sets up isolated config environment for testing config discovery.
///
/// Returns (`EnvLock`, `project_dir`, `fake_home`, `env_guards`) where `env_guards`
/// isolate `HOME` and platform-specific config paths (`XDG_CONFIG_HOME` on Unix,
/// `APPDATA`/`LOCALAPPDATA` on Windows) and remove explicit config selectors.
#[cfg(test)]
#[fixture]
fn isolated_config_env() -> anyhow::Result<(
    test_support::env_lock::EnvLock,
    tempfile::TempDir,
    tempfile::TempDir,
    Vec<EnvVarGuard>,
)> {
    use test_support::env_lock::EnvLock;
    let lock = EnvLock::acquire();
    let dir = tempfile::tempdir()?;
    let fake_home = tempfile::tempdir()?;

    #[cfg(unix)]
    let guards = vec![
        EnvVarGuard::set("HOME", fake_home.path().as_os_str()),
        EnvVarGuard::set("XDG_CONFIG_HOME", fake_home.path().as_os_str()),
        EnvVarGuard::remove(CONFIG_ENV_VAR),
        EnvVarGuard::remove(CONFIG_ENV_VAR_LEGACY),
    ];

    #[cfg(windows)]
    let guards = vec![
        EnvVarGuard::set("HOME", fake_home.path().as_os_str()),
        EnvVarGuard::set("APPDATA", fake_home.path().as_os_str()),
        EnvVarGuard::set("LOCALAPPDATA", fake_home.path().as_os_str()),
        EnvVarGuard::remove(CONFIG_ENV_VAR),
        EnvVarGuard::remove(CONFIG_ENV_VAR_LEGACY),
    ];

    Ok((lock, dir, fake_home, guards))
}

#[rstest]
#[case::no_file(false, true)]
#[case::file_present(true, false)]
fn collect_diag_file_layers_handles_project_file_presence(
    isolated_config_env: anyhow::Result<(
        test_support::env_lock::EnvLock,
        tempfile::TempDir,
        tempfile::TempDir,
        Vec<EnvVarGuard>,
    )>,
    #[case] create_file: bool,
    #[case] expect_empty: bool,
) -> anyhow::Result<()> {
    let (_lock, dir, _fake_home, _guards) = isolated_config_env?;

    if create_file {
        std::fs::write(dir.path().join(".netsuke.toml"), r"diag_json = true")
            .expect("write config");
    }

    let cli = cli_with_directory(dir.path());
    let layers = collect_diag_file_layers(&cli);

    if expect_empty {
        ensure!(layers.is_empty(), "expected no layers when file absent");
    } else {
        ensure!(
            !layers.is_empty(),
            "should include the project config layer when file present"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// diag_json_from_matches
// ---------------------------------------------------------------------------

#[test]
fn diag_json_from_matches_returns_discovered_when_no_cli_flag_set() {
    let app = Cli::command();
    let matches = app.get_matches_from(["netsuke"]);
    let cli = Cli::default();
    assert!(
        diag_json_from_matches(&cli, &matches, true),
        "should return the discovered value (true) when no CLI flag is set"
    );
    assert!(
        !diag_json_from_matches(&cli, &matches, false),
        "should return the discovered value (false) when no CLI flag is set"
    );
}

// ---------------------------------------------------------------------------
// push_file_layers
// ---------------------------------------------------------------------------

#[rstest]
#[case::empty_directory(false, 0)]
#[case::config_file_present(true, 1)]
fn push_file_layers_pushes_expected_layer_count(
    #[case] write_config: bool,
    #[case] expected_layers: usize,
) {
    use test_support::env_lock::EnvLock;
    let _lock = EnvLock::acquire();
    let dir = tempdir().expect("tempdir");
    let fake_home = tempdir().expect("fake home tempdir");
    if write_config {
        std::fs::write(dir.path().join(".netsuke.toml"), r#"theme = "unicode""#)
            .expect("write config");
    }
    let mut composer = MergeComposer::with_capacity(1);
    let mut errors = Vec::new();
    let _home_guard = EnvVarGuard::set("HOME", fake_home.path().as_os_str());
    #[cfg(unix)]
    let _xdg_config_home_guard = EnvVarGuard::set("XDG_CONFIG_HOME", fake_home.path().as_os_str());
    #[cfg(windows)]
    let _appdata_guard = EnvVarGuard::set("APPDATA", fake_home.path().as_os_str());
    #[cfg(windows)]
    let _local_appdata_guard = EnvVarGuard::set("LOCALAPPDATA", fake_home.path().as_os_str());
    let _config_guard = EnvVarGuard::remove(CONFIG_ENV_VAR);
    let _legacy_config_guard = EnvVarGuard::remove(CONFIG_ENV_VAR_LEGACY);
    let cli = cli_with_directory(dir.path());
    push_file_layers(&mut composer, &mut errors, &cli);
    assert!(errors.is_empty(), "no required errors expected");
    assert_eq!(
        composer.layers().len(),
        expected_layers,
        "unexpected number of layers pushed"
    );
}
