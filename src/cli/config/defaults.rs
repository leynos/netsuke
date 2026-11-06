//! Default values for the layered CLI configuration.
//!
//! [`CliConfig`]'s `Default` implementation and the default manifest path live
//! here rather than beside the schema so that the schema module stays within
//! the repository's module-size boundary. Both are part of the configuration
//! contract rather than an implementation detail: the `file` field's attribute
//! names [`default_manifest_path`], so a default applied to a layered key and
//! one produced by `CliConfig::default()` cannot drift apart.

use super::budget::{
    DEFAULT_MANIFEST_EVALUATION_FUEL, DEFAULT_MANIFEST_EXPANDED_ENTRIES,
    DEFAULT_MANIFEST_FOREACH_CARDINALITY, DEFAULT_MANIFEST_FUEL, DEFAULT_MANIFEST_RENDERED_BYTES,
    DEFAULT_MANIFEST_RENDERED_VALUE_BYTES, DEFAULT_MANIFEST_SOURCE_BYTES,
};
use super::{
    AccessibilityPolicy, CliConfig, ColourPolicy, CommandConfigs, EmojiPolicy, NoInput,
    ProgressPolicy,
};
use camino::Utf8PathBuf;

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            file: Self::default_manifest_path(),
            jobs: None,
            verbose: false,
            locale: None,
            fetch_allow_scheme: Vec::new(),
            env_allow_var: Vec::new(),
            env_block_var: Vec::new(),
            fetch_allow_host: Vec::new(),
            fetch_block_host: Vec::new(),
            fetch_default_deny: false,
            trust_project_fetch_policy: false,
            manifest_evaluation_fuel: DEFAULT_MANIFEST_EVALUATION_FUEL,
            manifest_fuel: DEFAULT_MANIFEST_FUEL,
            manifest_rendered_value_bytes: DEFAULT_MANIFEST_RENDERED_VALUE_BYTES,
            manifest_rendered_manifest_bytes: DEFAULT_MANIFEST_RENDERED_BYTES,
            manifest_source_bytes: DEFAULT_MANIFEST_SOURCE_BYTES,
            manifest_foreach_cardinality: DEFAULT_MANIFEST_FOREACH_CARDINALITY,
            manifest_expanded_entries: DEFAULT_MANIFEST_EXPANDED_ENTRIES,
            json: false,
            no_input: NoInput::default(),
            color: ColourPolicy::Auto,
            emoji: EmojiPolicy::Auto,
            progress: ProgressPolicy::Auto,
            accessibility: AccessibilityPolicy::Auto,
            default_targets: Vec::new(),
            shells: Vec::new(),
            cmds: CommandConfigs::default(),
        }
    }
}

impl CliConfig {
    /// Return the default manifest file path used when no file is supplied.
    ///
    /// Visible crate-wide because the Clap command definition and the runtime
    /// defaults both name it, and neither lives under this module.
    pub(crate) fn default_manifest_path() -> Utf8PathBuf {
        default_manifest_path()
    }
}

/// Return the default manifest file path, `Netsukefile` in the working directory.
pub(super) fn default_manifest_path() -> Utf8PathBuf {
    Utf8PathBuf::from("Netsukefile")
}
