//! Compile-pass fixture for the cached configuration-discovery API.
//!
//! Compiled by `tests/command_env_ui_tests.rs` against the `netsuke` rlib with
//! `--emit=metadata`. It proves an external caller can inject configuration
//! environment access, retain a discovery outcome, emit diagnostics, transfer
//! its cached layers, retrieve their bounded merge events without rediscovery,
//! and replay them through its own observer.

use netsuke::cli::{
    CachedMergeInput, Cli, ConfigEnvProvider, MergeEvent, MergeObserver,
    merge_with_cached_file_layers_with_observer, resolve_json_and_layers_outcome_with_env,
};
use std::{ffi::OsString, sync::Arc};

struct EmbeddedConfigEnv;

impl ConfigEnvProvider for EmbeddedConfigEnv {
    fn get(&self, _: &str) -> Option<OsString> { None }

    fn entries(&self) -> Vec<(OsString, OsString)> { Vec::new() }
}

struct EmbeddedObserver;

impl MergeObserver for EmbeddedObserver {
    fn observe(&mut self, event: MergeEvent) {
        match event {
            MergeEvent::DefaultsApplied
            | MergeEvent::DefaultsFailed
            | MergeEvent::EnvironmentFailed
            | MergeEvent::CliOverridesAbsent
            | MergeEvent::CliOverridesFailed => {}
            MergeEvent::FileLayersCollected { layer_count } => {
                let _ = layer_count;
            }
            MergeEvent::FileLayerCollectionFailed { error_count } => {
                let _ = error_count;
            }
            MergeEvent::FileLayerApplied { path_hash } => {
                let _ = path_hash;
            }
            MergeEvent::EnvironmentApplied { is_empty } => {
                let _ = is_empty;
            }
            MergeEvent::CliOverridesApplied { override_keys } => {
                let _ = override_keys;
            }
            MergeEvent::FetchPolicyReconciled { outcome } => {
                let _ = (
                    outcome.trusted_project_policy,
                    outcome.default_deny_decision.as_str(),
                );
            }
            MergeEvent::ValidationRejected { key, reason } => {
                let _ = (key, reason);
            }
        }
    }
}

fn main() {
    let localizer = Arc::from(netsuke::cli_localization::build_localizer(None));
    let (cli, matches) =
        netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer).expect("parse args");
    let env = EmbeddedConfigEnv;

    let (_, outcome) = resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    outcome.emit_diagnostics();
    let input = CachedMergeInput::new(&cli, &matches, &env, outcome.into_layers());
    let mut observer = EmbeddedObserver;
    let (merged, events) = merge_with_cached_file_layers_with_observer(input);
    for event in events {
        observer.observe(event);
    }
    let _ = merged;

    let _: Cli = Cli::default();
}
