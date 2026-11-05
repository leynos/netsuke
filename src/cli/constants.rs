//! Constants shared across CLI configuration modules.
//!
//! Owning these here keeps sibling modules such as `merge` and `environment`
//! free of dependencies on one another.

/// Prefix identifying environment variables that carry Netsuke configuration.
pub(super) const ENV_PREFIX: &str = "NETSUKE_";

/// Top-level configuration key that declares shell definitions.
///
/// Re-exported rather than restated: the schema, discovery, and the environment
/// layer must name the key identically, and the schema owns the spelling
/// because it also ships in the build script's compiled slice, which does not
/// compile this module.
pub(super) use crate::cli::config::shells::SHELLS_KEY;
