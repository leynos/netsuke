//! Property coverage for the cached diagnostic-to-merge discovery handoff.
//!
//! The fixed regression test in `merge_diag.rs` proves discovery reuse for a
//! single config file. These property tests check the same invariant over
//! generated config payloads: the cached handoff (resolve diagnostics, then
//! merge with the discovered layers) and a standalone merge (discover inside
//! the merge boundary) must produce the same effective configuration.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use netsuke::cli::ConfigEnvProvider;
use netsuke::cli_localization;
use proptest::prelude::*;
use std::{collections::HashMap, ffi::OsString, sync::Arc};
use tempfile::tempdir;

/// Deterministic in-memory environment for property tests.
///
/// This matches the crate-internal `TestEnv` double: a map-backed
/// [`ConfigEnvProvider`] exposing selector and merge lookups without touching
/// process-global state.
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

impl ConfigEnvProvider for TestEnv {
    fn get(&self, key: &str) -> Option<OsString> {
        self.values.get(key).cloned()
    }

    fn entries(&self) -> Vec<(OsString, OsString)> {
        self.values
            .iter()
            .map(|(key, value)| (OsString::from(*key), value.clone()))
            .collect()
    }
}

/// Render `(jobs, json)` as TOML content and write it to the temp config file.
fn render_toml(jobs: Option<usize>, json_pref: Option<bool>) -> String {
    let mut parts = Vec::new();
    if let Some(jobs_value) = jobs {
        parts.push(format!("jobs = {jobs_value}\n"));
    }
    if let Some(json_value) = json_pref {
        parts.push(format!("json = {json_value}\n"));
    }
    parts.concat()
}

proptest! {
    /// A cached handoff matches standalone merging across generated configs.
    ///
    /// `resolve_json_and_layers_outcome_with_env` discovers the file layers
    /// once; `merge_with_cached_file_layers` consumes those cached layers.
    /// The standalone `merge_with_config_and_env` discovers again, but both
    /// paths must agree on the merged `jobs` and `json`.
    #[test]
    fn cached_handoff_matches_standalone_merge(
        jobs in proptest::option::of(1usize..=64),
        json in any::<Option<bool>>(),
    ) {
        let temp_dir = tempdir().expect("create temporary config directory");
        let config_path = temp_dir.path().join("netsuke.toml");
        let config_dir =
            Dir::open_ambient_dir(temp_dir.path(), ambient_authority()).expect("open temp dir");
        let content = render_toml(jobs, json);
        config_dir
            .write("netsuke.toml", content.as_bytes())
            .expect("write generated config");

        let localizer = Arc::from(cli_localization::build_localizer(None));
        let (cli, matches) =
            netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
                .expect("parse CLI");
        let selector = config_path.to_string_lossy().into_owned();
        let env = TestEnv::default().with_var("NETSUKE_CONFIG", selector.as_str());

        let (json_result, outcome) =
            netsuke::cli::resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
        let resolved_json = json_result.expect("resolve diagnostic mode");
        let cached_merged =
            netsuke::cli::merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers())
                .expect("merge cached layers");

        // Standalone merge: discovers again inside the merge boundary.
        let standalone_merged = netsuke::cli::merge_with_config_and_env(&cli, &matches, &env)
            .expect("standalone merge must succeed");

        prop_assert_eq!(
            cached_merged.jobs, standalone_merged.jobs,
            "merged jobs should match between cached handoff and standalone merge"
        );
        prop_assert_eq!(
            cached_merged.json, standalone_merged.json,
            "merged json should match between cached handoff and standalone merge"
        );
        prop_assert_eq!(
            cached_merged.json, resolved_json,
            "diagnostic resolution and the merged config should agree on json"
        );
        prop_assert_eq!(
            cached_merged.jobs, jobs,
            "configured jobs should survive the cached handoff"
        );
    }
}

/// The generated config file is always parseable and yields only the given keys.
#[test]
fn rendered_config_file_is_parseable_toml() -> Result<()> {
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    let config_dir = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())
        .context("open temp config directory")?;
    config_dir
        .write("netsuke.toml", b"jobs = 3\njson = true\n")
        .context("write config")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) =
        netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer).context("parse CLI")?;
    let selector = config_path.to_string_lossy().into_owned();
    let env = TestEnv::default().with_var("NETSUKE_CONFIG", selector.as_str());

    let merged = netsuke::cli::merge_with_config_and_env(&cli, &matches, &env)
        .context("merge generated config")?;
    ensure!(
        merged.jobs == Some(3) && merged.json,
        "merged result should carry the generated jobs and json"
    );
    Ok(())
}
