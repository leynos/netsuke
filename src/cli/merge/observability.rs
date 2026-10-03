//! Explicit observability adapter for configuration merging.
//!
//! Merge queries return [`MergeEvent`] values to their application boundary.
//! Production replays them through [`TracingMergeObserver`], while direct
//! callers remain free of logging side effects.

use serde_json::Value;

use crate::stdlib::FetchPolicyReconciliationOutcome;

use super::super::config::NO_INPUT_VALIDATION_REASON;

/// Fixed reason reported when a merged parallel job count is out of range.
const JOBS_VALIDATION_REASON: &str = "job count is outside the supported range";

/// Return whether a configuration-layer object contains no supplied settings.
pub(crate) fn is_empty_configuration_value(value: &Value) -> bool {
    matches!(value, Value::Object(map) if map.is_empty())
}

/// Return the bounded observability reason for a known validation key.
pub(crate) fn validation_rejection_reason(key: &str) -> Option<&'static str> {
    match key {
        "no_input" => Some(NO_INPUT_VALIDATION_REASON),
        "jobs" => Some(JOBS_VALIDATION_REASON),
        _ => None,
    }
}

/// A bounded event emitted by an explicitly supplied configuration observer.
///
/// The event surface intentionally excludes configuration values and raw
/// paths. It lets the application decide whether and how to record merge
/// diagnostics without coupling configuration queries to a global subscriber.
#[derive(Debug)]
pub enum MergeEvent {
    /// The defaults layer was added to the composition.
    DefaultsApplied,
    /// Serializing the defaults layer failed.
    DefaultsFailed,
    /// Configuration file discovery completed before file layers are added.
    FileLayersCollected {
        /// Number of discovered file layers when collection succeeded.
        layer_count: usize,
    },
    /// Configuration file discovery produced errors.
    FileLayerCollectionFailed {
        /// Number of discovery errors retained for composition.
        error_count: usize,
    },
    /// A discovered file layer was added using its bounded path identifier.
    FileLayerApplied {
        /// Bounded hash of the file path, when the layer originates from a file.
        path_hash: Option<String>,
    },
    /// The environment layer was added to the composition.
    EnvironmentApplied {
        /// Whether the extracted environment layer contains no settings.
        is_empty: bool,
    },
    /// Extracting the environment layer failed.
    EnvironmentFailed,
    /// Explicit CLI overrides were added without their values.
    CliOverridesApplied {
        /// Bounded dot-separated paths for explicitly overridden leaf settings.
        override_keys: Vec<String>,
    },
    /// No CLI setting was explicitly overridden.
    CliOverridesAbsent,
    /// Serializing CLI overrides failed.
    CliOverridesFailed,
    /// Fetch-policy reconciliation completed after a successful generic merge.
    FetchPolicyReconciled {
        /// Bounded decisions and grant counts, excluding policy values.
        outcome: FetchPolicyReconciliationOutcome,
    },
    /// Post-merge validation rejected a known configuration setting.
    ValidationRejected {
        /// Name of the rejected setting.
        key: String,
        /// Fixed explanation of the rejected setting's invalid state.
        reason: &'static str,
    },
}

/// Receives bounded configuration-merge events at an explicit application seam.
pub trait MergeObserver {
    /// Receive one event from a configuration merge attempt.
    fn observe(&mut self, event: MergeEvent);
}

/// Application adapter that records merge events through `tracing`.
#[derive(Debug, Default)]
pub struct TracingMergeObserver;

impl MergeObserver for TracingMergeObserver {
    /// Record one bounded merge event through its matching tracing field set.
    fn observe(&mut self, event: MergeEvent) {
        record_default_event(&event);
        record_file_event(&event);
        record_environment_event(&event);
        record_cli_event(&event);
        record_validation_event(&event);
        record_fetch_policy_event(&event);
    }
}

/// Record only fixed decisions and counts from a completed reconciliation.
fn record_fetch_policy_event(event: &MergeEvent) {
    if let MergeEvent::FetchPolicyReconciled { outcome } = event {
        tracing::debug!(
            trusted_project_policy = outcome.trusted_project_policy,
            project_request_present = outcome.project_request_present,
            default_deny_decision = outcome.default_deny_decision.as_str(),
            requested_scheme_grants = outcome.requested_scheme_grants,
            accepted_scheme_grants = outcome.accepted_scheme_grants,
            ignored_scheme_grants = outcome.ignored_scheme_grants,
            requested_host_grants = outcome.requested_host_grants,
            accepted_host_grants = outcome.accepted_host_grants,
            ignored_host_grants = outcome.ignored_host_grants,
            "reconciled fetch policy"
        );
    }
}

/// Emit one bounded defaults-layer event from a retained message.
///
/// The macro is the sole statement so the expansion is charged here rather than
/// added to the caller's cognitive complexity.
fn debug_defaults_layer_from_fields(message: &'static str) {
    tracing::debug!(layer = "defaults", message);
}

/// Record a bounded defaults-layer event when `event` represents one.
fn record_default_event(event: &MergeEvent) {
    match event {
        MergeEvent::DefaultsApplied => {
            debug_defaults_layer_from_fields("applied default configuration layer");
        }
        MergeEvent::DefaultsFailed => {
            debug_defaults_layer_from_fields("default configuration layer failed");
        }
        _ => {}
    }
}

/// Emit the collected file-layer event with its retained layer count.
fn debug_file_layers_collected_from_fields(layer_count: usize) {
    tracing::debug!(
        layer = "file",
        layer_count,
        "collected configuration file layers"
    );
}

/// Emit the file-layer collection failure with its retained error count.
fn debug_file_layer_collection_failed_from_fields(error_count: usize) {
    tracing::debug!(
        layer = "file",
        error_count,
        "configuration file layer collection failed"
    );
}

/// Emit the applied file-layer event with its bounded path hash.
fn debug_file_layer_applied_from_fields(path_hash: Option<&String>) {
    tracing::debug!(layer = "file", path_hash = ?path_hash, "applied configuration file layer");
}

/// Record a bounded file-layer event when `event` represents one.
fn record_file_event(event: &MergeEvent) {
    match event {
        MergeEvent::FileLayersCollected { layer_count } => {
            debug_file_layers_collected_from_fields(*layer_count);
        }
        MergeEvent::FileLayerCollectionFailed { error_count } => {
            debug_file_layer_collection_failed_from_fields(*error_count);
        }
        MergeEvent::FileLayerApplied { path_hash } => {
            debug_file_layer_applied_from_fields(path_hash.as_ref());
        }
        _ => {}
    }
}

/// Emit the merged environment-layer event with its retained emptiness flag.
fn debug_environment_applied_from_fields(is_empty: bool) {
    tracing::debug!(
        layer = "environment",
        is_empty,
        "merged environment configuration layer"
    );
}

/// Emit the environment-layer failure event.
fn debug_environment_failed_from_fields() {
    tracing::debug!(
        layer = "environment",
        "environment configuration layer failed"
    );
}

/// Record a bounded environment-layer event when `event` represents one.
fn record_environment_event(event: &MergeEvent) {
    match event {
        MergeEvent::EnvironmentApplied { is_empty } => {
            debug_environment_applied_from_fields(*is_empty);
        }
        MergeEvent::EnvironmentFailed => {
            debug_environment_failed_from_fields();
        }
        _ => {}
    }
}

/// Emit the applied CLI-override event with its bounded override keys.
fn debug_cli_overrides_applied_from_fields(override_keys: &[String]) {
    tracing::debug!(layer = "cli", override_keys = ?override_keys, "applied CLI override layer");
}

/// Emit one bounded CLI-layer event from a retained message.
fn debug_cli_layer_from_fields(message: &'static str) {
    tracing::debug!(layer = "cli", message);
}

/// Record a bounded CLI-layer event when `event` represents one.
fn record_cli_event(event: &MergeEvent) {
    match event {
        MergeEvent::CliOverridesApplied { override_keys } => {
            debug_cli_overrides_applied_from_fields(override_keys);
        }
        MergeEvent::CliOverridesAbsent => {
            debug_cli_layer_from_fields("no explicit CLI overrides supplied");
        }
        MergeEvent::CliOverridesFailed => {
            debug_cli_layer_from_fields("CLI override layer failed");
        }
        _ => {}
    }
}

/// Record a validation event when `event` represents a rejected setting.
fn record_validation_event(event: &MergeEvent) {
    if let MergeEvent::ValidationRejected { key, reason } = event {
        tracing::debug!(key, reason, "validation rejected merged configuration");
    }
}

/// Collect bounded leaf paths from an override object without recording values.
pub(crate) fn collect_override_leaf_paths(value: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    collect_leaf_paths(value, "", &mut paths);
    paths
}

/// Traverse one nested override value and append only paths ending at leaves.
fn collect_leaf_paths(value: &Value, prefix: &str, paths: &mut Vec<String>) {
    if let Value::Object(map) = value {
        for (key, nested_value) in map {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            collect_leaf_paths(nested_value, &path, paths);
        }
    } else if !prefix.is_empty() {
        paths.push(prefix.to_owned());
    }
}
