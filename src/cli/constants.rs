//! Constants shared across CLI configuration modules.
//!
//! Owning these here keeps sibling modules such as `merge` and `environment`
//! free of dependencies on one another.

/// Prefix identifying environment variables that carry Netsuke configuration.
pub(super) const ENV_PREFIX: &str = "NETSUKE_";
