//! Integration coverage for early diagnostic JSON preference resolution.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use netsuke::cli_localization;
use std::{collections::HashMap, ffi::OsString, sync::Arc};
use tempfile::tempdir;

#[derive(Default)]
struct TestEnv {
    values: HashMap<&'static str, OsString>,
}

impl TestEnv {
    fn with_var(mut self, name: &'static str, value: impl Into<OsString>) -> Self {
        self.values.insert(name, value.into());
        self
    }
}

impl netsuke::cli::ConfigEnvProvider for TestEnv {
    fn get(&self, key: &str) -> Option<OsString> {
        self.values.get(key).cloned()
    }
}

#[test]
fn resolve_merged_diag_json_honours_injected_env() -> Result<()> {
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    let config_dir = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())
        .context("open temporary config directory")?;
    config_dir
        .write("netsuke.toml", b"diag_json = false\n")
        .context("write netsuke.toml")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let config_arg = config_path.to_string_lossy().into_owned();
    let (cli, matches) =
        netsuke::cli::parse_with_localizer_from(["netsuke", "--config", &config_arg], &localizer)
            .context("parse CLI args for injected diag_json env")?;
    let env = TestEnv::default().with_var("NETSUKE_DIAG_JSON", "1");

    ensure!(
        netsuke::cli::resolve_merged_diag_json_with_env(&cli, &matches, &env)?,
        "injected NETSUKE_DIAG_JSON should override file config",
    );

    Ok(())
}
