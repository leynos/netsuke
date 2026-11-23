//! Layered CLI configuration schema.
//!
//! [`CliConfig`] is the single typed schema used for configuration discovery
//! and merging. It captures global CLI settings, trusted shell definitions, and
//! per-subcommand defaults under the `cmds` namespace.
pub(super) use super::validation::validation_error;
use crate::host_pattern::HostPattern;
use camino::Utf8PathBuf;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

mod budget;
mod defaults;
mod no_input;
mod policies;
pub mod shells;
mod validation;
pub use no_input::NoInput;
pub use policies::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
pub(super) mod policy_definitions;
use budget::{
    DEFAULT_MANIFEST_EVALUATION_FUEL, DEFAULT_MANIFEST_EXPANDED_BYTES,
    DEFAULT_MANIFEST_EXPANDED_ENTRIES, DEFAULT_MANIFEST_FOREACH_CARDINALITY, DEFAULT_MANIFEST_FUEL,
    DEFAULT_MANIFEST_RENDERED_BYTES, DEFAULT_MANIFEST_RENDERED_VALUE_BYTES,
    DEFAULT_MANIFEST_SOURCE_BYTES,
};
use defaults::default_manifest_path;
use shells::ShellDefinition;

/// Layered defaults for the `build` subcommand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BuildConfig {
    /// Default targets used when the user does not pass any targets.
    #[serde(default)]
    pub targets: Vec<String>,
}

/// Layered defaults for the `check` subcommand.
///
/// Lint policy belongs in configuration rather than only on the command line:
/// a project's rule selection is a property of the project, and repeating it in
/// every continuous-integration invocation is how the two drift apart.
#[cfg(feature = "lint")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CheckConfig {
    /// Rule and category severity selectors, as `NAME=SEVERITY`.
    #[serde(default)]
    pub rule: Vec<String>,

    /// Severity at which findings fail the command.
    #[serde(default)]
    pub fail_on: Option<String>,

    /// Maximum findings to report; `0` reports all of them.
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Subcommand-specific layered defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CommandConfigs {
    /// Configuration that applies only to the `build` subcommand.
    #[serde(default)]
    pub build: BuildConfig,

    /// Configuration that applies only to the `check` subcommand.
    #[cfg(feature = "lint")]
    #[serde(default)]
    pub check: CheckConfig,
}

/// Authoritative schema for layered CLI configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, OrthoConfig)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each Boolean maps to an independent layered configuration key"
)]
#[ortho_config(prefix = "NETSUKE", post_merge_hook)]
pub struct CliConfig {
    /// Path to the Netsuke manifest file to use.
    #[ortho_config(default = default_manifest_path(), cli(value_name = "FILE"))]
    pub file: Utf8PathBuf,

    /// Set the number of parallel build jobs.
    #[ortho_config(cli(value_name = "N"))]
    pub jobs: Option<usize>,

    /// Enable verbose diagnostic logging and completion timing summaries.
    #[ortho_config(default = false)]
    pub verbose: bool,

    /// Locale tag for CLI copy (for example: en-US, es-ES).
    #[ortho_config(cli(value_name = "LOCALE"))]
    pub locale: Option<String>,

    /// Additional URL schemes allowed for the `fetch` helper.
    #[ortho_config(merge_strategy = "append", cli(value_name = "SCHEME"))]
    #[serde(default)]
    pub fetch_allow_scheme: Vec<String>,

    /// Environment variables permitted for the manifest `env()` helper.
    #[ortho_config(merge_strategy = "append", cli(value_name = "VAR"))]
    #[serde(default)]
    pub env_allow_var: Vec<String>,

    /// Environment variables always blocked for the manifest `env()` helper.
    #[ortho_config(merge_strategy = "append", cli(value_name = "VAR"))]
    #[serde(default)]
    pub env_block_var: Vec<String>,

    /// Hostnames permitted when default deny is enabled.
    #[ortho_config(merge_strategy = "append", cli(value_name = "HOST"))]
    #[serde(default)]
    pub fetch_allow_host: Vec<HostPattern>,

    /// Hostnames that are always blocked.
    #[ortho_config(merge_strategy = "append", cli(value_name = "HOST"))]
    #[serde(default)]
    pub fetch_block_host: Vec<HostPattern>,

    /// Deny all hosts by default; only allow the declared allowlist.
    #[ortho_config(default = false)]
    pub fetch_default_deny: bool,

    /// Allow project configuration to widen fetch-policy grants.
    #[ortho_config(default = false)]
    pub trust_project_fetch_policy: bool,

    /// Maximum `MiniJinja` instructions allocated to one manifest evaluation.
    #[ortho_config(
        default = DEFAULT_MANIFEST_EVALUATION_FUEL,
        cli(value_name = "FUEL")
    )]
    pub manifest_evaluation_fuel: u64,

    /// Maximum `MiniJinja` instructions allocated across one manifest.
    #[ortho_config(default = DEFAULT_MANIFEST_FUEL, cli(value_name = "FUEL"))]
    pub manifest_fuel: u64,

    /// Maximum bytes emitted by one rendered manifest value.
    #[ortho_config(
        default = DEFAULT_MANIFEST_RENDERED_VALUE_BYTES,
        cli(value_name = "BYTES")
    )]
    pub manifest_rendered_value_bytes: usize,

    /// Maximum aggregate bytes emitted across all rendered manifest values.
    #[ortho_config(
        default = DEFAULT_MANIFEST_RENDERED_BYTES,
        cli(value_name = "BYTES")
    )]
    pub manifest_rendered_manifest_bytes: usize,

    /// Maximum template and macro-import source bytes consumed per manifest.
    #[ortho_config(
        default = DEFAULT_MANIFEST_SOURCE_BYTES,
        cli(value_name = "BYTES")
    )]
    pub manifest_source_bytes: usize,

    /// Maximum values consumed from each manifest `foreach` iterator.
    #[ortho_config(
        default = DEFAULT_MANIFEST_FOREACH_CARDINALITY,
        cli(value_name = "COUNT")
    )]
    pub manifest_foreach_cardinality: usize,

    /// Maximum expanded target and action entries per manifest.
    #[ortho_config(
        default = DEFAULT_MANIFEST_EXPANDED_ENTRIES,
        cli(value_name = "COUNT")
    )]
    pub manifest_expanded_entries: usize,

    /// Maximum modelled bytes retained by expanded `foreach` entries.
    #[ortho_config(
        default = DEFAULT_MANIFEST_EXPANDED_BYTES,
        cli(value_name = "BYTES")
    )]
    pub manifest_expanded_bytes: usize,

    /// Emit machine-readable JSON output.
    #[ortho_config(default = false)]
    pub json: bool,

    /// Never read interactive input.
    #[ortho_config(skip_cli)]
    pub no_input: NoInput,

    /// Preferred colour policy.
    #[ortho_config(skip_cli)]
    pub color: ColourPolicy,

    /// Preferred emoji policy.
    #[ortho_config(skip_cli)]
    pub emoji: EmojiPolicy,

    /// Preferred progress policy.
    #[ortho_config(skip_cli)]
    pub progress: ProgressPolicy,

    /// Preferred accessibility policy.
    #[ortho_config(skip_cli)]
    pub accessibility: AccessibilityPolicy,

    /// Compatibility alias for default build targets at the config root.
    #[ortho_config(merge_strategy = "append", cli(value_name = "TARGET"))]
    #[serde(default)]
    pub default_targets: Vec<String>,

    /// Trusted shell definitions declared by an operator configuration file.
    ///
    /// Shell definitions grant executable authority, so the key is absent from
    /// the command line and from the environment; discovery admits it only in
    /// an operator-owned root. Layers concatenate in merge order, and the
    /// post-merge hook then rejects built-in and repeated names across the
    /// whole registry.
    #[ortho_config(skip_cli, merge_strategy = "append")]
    #[serde(default)]
    pub shells: Vec<ShellDefinition>,

    /// Per-subcommand defaults.
    #[ortho_config(skip_cli)]
    #[serde(default)]
    pub cmds: CommandConfigs,
}

/// Fixed reason reported when merged configuration enables interactive input.
///
/// Defined here rather than in [`validation`] because
/// `merge_observability` maps the rejected `no_input` key to this text without
/// depending on the validation slice.
pub(crate) const NO_INPUT_VALIDATION_REASON: &str =
    "no_input = false is unsupported because Netsuke has no interactive mode";
#[cfg(test)]
mod tests;
