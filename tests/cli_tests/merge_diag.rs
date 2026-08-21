//! Integration coverage for early JSON preference resolution.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use netsuke::cli_localization;
use std::cell::Cell;
use std::{collections::HashMap, ffi::OsString, sync::Arc};
use tempfile::tempdir;

#[test]
fn resolve_merged_json_honours_injected_env() -> Result<()> {
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    let config_dir = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())
        .context("open temporary config directory")?;
    config_dir
        .write("netsuke.toml", b"json = false\n")
        .context("write netsuke.toml")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let config_arg = config_path.to_string_lossy().into_owned();
    let (cli, matches) =
        netsuke::cli::parse_with_localizer_from(["netsuke", "--config", &config_arg], &localizer)
            .context("parse CLI args for injected JSON env")?;
    let env = TestEnv::default().with_var("NETSUKE_JSON", "1");

    ensure!(
        netsuke::cli::resolve_merged_json_with_env(&cli, &matches, &env)?,
        "injected NETSUKE_JSON should override file config",
    );

    Ok(())
}

/// Cached layers keep the file values found during diagnostic resolution.
#[test]
fn cached_merge_does_not_reload_discovered_config() -> Result<()> {
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    let config_dir = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())
        .context("open temporary config directory")?;
    config_dir
        .write("netsuke.toml", b"jobs = 13\n")
        .context("write initial config")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let config_arg = config_path.to_string_lossy().into_owned();
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(
        ["netsuke", "--config", config_arg.as_str()],
        &localizer,
    )
    .context("parse CLI args for cached merge")?;
    let env = TestEnv::default();
    let (json, outcome) =
        netsuke::cli::resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    ensure!(!json?, "initial configuration should not enable JSON mode");

    config_dir
        .write("netsuke.toml", b"jobs = 29\n")
        .context("change config after discovery")?;
    let merged =
        netsuke::cli::merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers())?;

    ensure!(
        merged.jobs == Some(13),
        "cached merge should retain the initially discovered file value"
    );
    Ok(())
}

/// The diagnostic pass and cached merge share one `NETSUKE_CONFIG` lookup.
///
/// Discovery happens once during diagnostic resolution; the subsequent merge
/// consumes the retained layers instead of re-reading the selector. This is
/// the single-lookup guarantee that avoids double environment I/O on startup.
#[test]
fn diag_then_cached_merge_reads_config_selector_once() -> Result<()> {
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    let config_dir = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())
        .context("open temporary config directory")?;
    config_dir
        .write("netsuke.toml", b"jobs = 13\njson = true\n")
        .context("write config")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) =
        netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer).context("parse CLI")?;
    let selector = config_path.as_os_str().to_owned();
    let env = CountingConfigEnv::default().with_var("NETSUKE_CONFIG", selector);

    let (json, outcome) =
        netsuke::cli::resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    ensure!(
        json?,
        "the discovered file layer should enable JSON diagnostics"
    );
    let merged =
        netsuke::cli::merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers())
            .context("merge the cached layers")?;

    ensure!(
        merged.jobs == Some(13) && merged.json,
        "the merge should consume the discovered config layer"
    );
    ensure!(
        env.config_lookups() == 1,
        "diagnostic resolution and cached merge must share one NETSUKE_CONFIG lookup, found {}",
        env.config_lookups()
    );
    Ok(())
}

#[derive(Default)]
struct TestEnv {
    values: HashMap<&'static str, OsString>,
}

impl netsuke::cli::ConfigEnvProvider for TestEnv {
    fn get(&self, key: &str) -> Option<OsString> {
        self.values.get(key).cloned()
    }

    fn entries(&self) -> Vec<(OsString, OsString)> {
        Vec::new()
    }
}

/// [`ConfigEnvProvider`] that counts `NETSUKE_CONFIG` selector lookups.
///
/// This lets the cached hand-off regression assert that diagnostic resolution
/// and the full merge observe exactly one selector read, so startup does not
/// pay a second environment lookup or file load.
#[derive(Default)]
struct CountingConfigEnv {
    values: HashMap<&'static str, OsString>,
    config_lookups: Cell<usize>,
}

impl CountingConfigEnv {
    fn with_var(mut self, name: &'static str, value: impl Into<OsString>) -> Self {
        self.values.insert(name, value.into());
        self
    }

    const fn config_lookups(&self) -> usize {
        self.config_lookups.get()
    }
}

impl netsuke::cli::ConfigEnvProvider for CountingConfigEnv {
    fn get(&self, key: &str) -> Option<OsString> {
        if key == "NETSUKE_CONFIG" {
            self.config_lookups.set(self.config_lookups.get() + 1);
        }
        self.values.get(key).cloned()
    }

    fn entries(&self) -> Vec<(OsString, OsString)> {
        Vec::new()
    }
}

impl TestEnv {
    fn with_var(mut self, name: &'static str, value: impl Into<OsString>) -> Self {
        self.values.insert(name, value.into());
        self
    }
}
