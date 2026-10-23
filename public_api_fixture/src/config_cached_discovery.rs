//! Type-check the cached configuration-discovery API.
//!
//! An external caller must be able to inject configuration environment access,
//! retain a discovery outcome, emit diagnostics, transfer its cached layers,
//! retrieve their bounded merge events without rediscovery, and replay them
//! through its own observer.

use netsuke::cli::{
    CachedMergeInput, Cli, ConfigEnvProvider, MergeEvent, MergeObserver,
    merge_with_cached_file_layers_with_observer, resolve_json_and_layers_outcome_with_env,
};
use std::{ffi::OsString, sync::Arc};

/// Supply an empty injected configuration environment.
pub struct EmbeddedConfigEnv;

impl ConfigEnvProvider for EmbeddedConfigEnv {
    fn get(&self, _: &str) -> Option<OsString> {
        None
    }

    fn entries(&self) -> Vec<(OsString, OsString)> {
        Vec::new()
    }
}

/// Observe every public merge event, destructuring each payload.
pub struct EmbeddedObserver;

impl MergeObserver for EmbeddedObserver {
    fn observe(&mut self, event: MergeEvent) {
        // The match is exhaustive and names every field on purpose: an
        // external crate can only write it while each variant and field stays
        // public.
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

/// Run discovery once, then merge and replay from the cached outcome.
pub fn replay_cached_discovery() {
    let localizer = Arc::from(netsuke::cli_localization::build_localizer(None));
    let Ok((cli, matches)) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
    else {
        return;
    };
    let env = EmbeddedConfigEnv;

    let (_, outcome) = resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    outcome.emit_diagnostics();
    let input = CachedMergeInput::new(&cli, &matches, &env, outcome.into_layers());
    let mut observer = EmbeddedObserver;
    let (merged, events) = merge_with_cached_file_layers_with_observer(input);
    for event in events {
        observer.observe(event);
    }
    drop(merged);
}

/// Construct the default CLI value an embedder may start from.
#[must_use]
pub fn default_cli() -> Cli {
    Cli::default()
}
