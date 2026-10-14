//! Type-check Netsuke's public cached configuration API.
//!
//! Exercises the `ConfigEnvProvider` boundary exactly as an external embedder
//! uses it: resolve diagnostics once, retain the discovered layers, then merge
//! from the cached outcome.

use netsuke::{cli, cli_localization};
use std::{ffi::OsString, sync::Arc};

/// Supply an empty injected environment for the public configuration API.
pub struct FixtureEnvProvider;

impl cli::ConfigEnvProvider for FixtureEnvProvider {
    fn get(&self, _: &str) -> Option<OsString> {
        None
    }

    fn entries(&self) -> Vec<(OsString, OsString)> {
        Vec::new()
    }
}

/// Compose diagnostic discovery and cached-layer merging as an external caller.
pub fn compose_cached_configuration_flow() {
    let localizer = Arc::from(cli_localization::build_localizer(None));
    let Ok((parsed, matches)) = cli::parse_with_localizer_from(["netsuke", "generate"], &localizer)
    else {
        return;
    };
    let env = FixtureEnvProvider;

    drop(cli::resolve_merged_json_with_env(&parsed, &matches, &env));
    let (result, outcome) = cli::resolve_json_and_layers_outcome_with_env(&parsed, &matches, &env);
    outcome.emit_diagnostics();
    let layers = outcome.into_layers();
    drop(result);
    drop(cli::merge_with_cached_file_layers(
        &parsed, &matches, &env, layers,
    ));
    drop(cli::merge_with_config_and_env(&parsed, &matches, &env));
}
