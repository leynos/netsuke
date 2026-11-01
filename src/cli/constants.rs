//! Constants shared across CLI configuration modules.
//!
//! Owning these here keeps sibling modules such as `merge` and `environment`
//! free of dependencies on one another.

/// Prefix identifying environment variables that carry Netsuke configuration.
pub(super) const ENV_PREFIX: &str = "NETSUKE_";

/// Top-level configuration key that declares shell definitions.
///
/// Shell definitions grant executable authority, so discovery treats this key
/// as a trust boundary rather than an ordinary configuration value: only an
/// operator-owned root may declare it.
pub(super) const SHELLS_KEY: &str = "shells";
