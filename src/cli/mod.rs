//! Command-line parsing plus layered CLI configuration support.
//!
//! The parser-facing [`Cli`] type remains responsible for user-facing command
//! syntax, while [`CliConfig`] is the authoritative OrthoConfig-derived schema
//! used to merge defaults, configuration files, environment variables, and CLI
//! overrides into the runtime shape consumed by the runner.

use ortho_config::OrthoError;
use std::sync::Arc;

pub mod config;
mod constants;
mod diag;
mod discovery;
mod environment;
mod help;
mod merge;
mod merge_input;
mod merge_observability;
mod parser;
mod parsing;
mod policy_values;
mod release_help;
#[cfg(test)]
pub(crate) mod test_support;
mod value_parser;

pub use config::{AccessibilityPolicy, CliConfig, ColourPolicy, EmojiPolicy, ProgressPolicy};
pub use diag::{
    resolve_json_and_layers_outcome_with_env, resolve_merged_json, resolve_merged_json_with_env,
};
/// Cached file layers, discovery errors, and deferred diagnostics from one
/// configuration discovery pass.
pub use discovery::DiscoveredLayers;
/// Side-effect-free diagnostic-mode resolution paired with cached discovery.
pub use discovery::DiscoveryOutcome;
/// Environment access seam for configuration discovery and merging.
pub use discovery::EnvProvider as ConfigEnvProvider;
/// Process-backed configuration environment adapter for production callers.
pub use discovery::StdEnvProvider as ConfigStdEnvProvider;
/// Record the discovery metric series for an already-timed phase.
pub use discovery::record_discovery_outcome;
pub use help::{HelpArgs, HelpTopic};
pub use merge::{
    merge_with_cached_file_layers, merge_with_cached_file_layers_with_observer, merge_with_config,
    merge_with_config_and_env,
};
/// Input for an observer-enabled merge using previously discovered layers.
pub use merge_input::CachedMergeInput;
/// Bounded events and the production tracing adapter for observer-enabled merges.
pub use merge_observability::{MergeEvent, MergeObserver, TracingMergeObserver};
pub(crate) use parser::configured_command;
pub use parser::{
    BuildArgs, Cli, Commands, GraphArgs, json_hint_from_args, locale_hint_from_args,
    parse_with_localizer_from,
};
pub use release_help::ReleaseHelpCli;

/// Counter recording configuration discovery passes by bounded outcome.
pub const DISCOVERY_TOTAL: &str = "netsuke_cli_config_discovery_total";
/// Histogram recording configuration discovery duration in seconds.
pub const DISCOVERY_DURATION: &str = "netsuke_cli_config_discovery_duration_seconds";
/// Bounded outcome values admitted on the discovery counter series.
pub const DISCOVERY_OUTCOME_VALUES: [&str; 2] = ["success", "error"];

/// Maximum number of jobs accepted by the CLI.
pub(super) const MAX_JOBS: usize = 64;

/// Build an `OrthoError::Validation` error for `key` with the given message.
pub(super) fn validation_error(key: &str, message: &str) -> Arc<OrthoError> {
    Arc::new(OrthoError::Validation {
        key: key.to_owned(),
        message: message.to_owned(),
    })
}

#[cfg(test)]
#[path = "merge_logging_proptests.rs"]
mod merge_logging_proptests;
