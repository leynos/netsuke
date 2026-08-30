//! Command-line parsing plus layered CLI configuration support.
//!
//! `command` owns the user-facing [`Cli`] schema and default-command behaviour,
//! while `parser` localises that schema before parsing. [`CliConfig`]
//! is the authoritative OrthoConfig-derived schema used to merge defaults,
//! configuration files, environment variables, and CLI overrides into the
//! runtime shape consumed by the runner.
//!
//! The module is split so that `build.rs` can compile the Clap schema alone.
//! `command`, [`config`], `help`, and `validation` form that self-contained
//! slice; every other submodule here is library-only. See the note in
//! `build.rs`.

mod command;
pub mod config;
mod constants;
mod diag;
mod discovery;
mod environment;
mod help;
mod merge;
mod merge_input;
mod merge_observability;
mod merge_subcommands;
mod parser;
mod parsing;
mod policy_values;
mod preferences;
mod release_help;
#[cfg(test)]
pub(crate) mod test_support;
mod validation;
mod value_parser;

pub use command::{
    BuildArgs, CheckArgs, Cli, Commands, DEFAULT_FAIL_ON, DEFAULT_FINDING_LIMIT, GraphArgs,
};
pub use config::{
    AccessibilityPolicy, CheckConfig, CliConfig, ColourPolicy, EmojiPolicy, ProgressPolicy,
};
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
/// Input for an event-collecting merge using previously discovered layers.
pub use merge_input::CachedMergeInput;
/// Bounded events and the production tracing adapter for application-side replay.
pub use merge_observability::{MergeEvent, MergeObserver, TracingMergeObserver};
pub(crate) use parser::configured_command;
pub use parser::{json_hint_from_args, locale_hint_from_args, parse_with_localizer_from};
pub use release_help::ReleaseHelpCli;

/// Counter recording configuration discovery passes by bounded outcome.
pub const DISCOVERY_TOTAL: &str = "netsuke_cli_config_discovery_total";
/// Histogram recording configuration discovery duration in seconds.
pub const DISCOVERY_DURATION: &str = "netsuke_cli_config_discovery_duration_seconds";
/// Bounded outcome values admitted on the discovery counter series.
pub const DISCOVERY_OUTCOME_VALUES: [&str; 2] = ["success", "error"];
/// Counter recording rejected UTF-8-only CLI path values by source and reason.
pub const PATH_VALIDATION_TOTAL: &str = "netsuke_cli_path_validation_total";
/// Bounded source values admitted on the CLI path-validation counter series.
pub const PATH_VALIDATION_SOURCE_VALUES: [&str; 2] = ["file", "directory"];
/// Bounded rejection reasons admitted on the CLI path-validation counter series.
pub const PATH_VALIDATION_REASON_VALUES: [&str; 1] = ["non_utf8"];
#[cfg(test)]
#[path = "merge_logging_proptests.rs"]
mod merge_logging_proptests;
