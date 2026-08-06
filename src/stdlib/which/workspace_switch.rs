//! The `NETSUKE_WHICH_WORKSPACE` opt-out switch for the workspace fallback.
//!
//! A leaf module by design: `env::EnvSnapshot::capture` reads the variable at
//! the resolver's ambient boundary and stores the raw result as snapshot
//! data, and `lookup::workspace` classifies that data when deciding whether
//! to search. Placing the name and the pure classifier here keeps both
//! consumers pointing downward — the earlier arrangement had `env` calling
//! back into `lookup::workspace`, a module cycle.

use std::env;

/// The variable a user sets to switch the workspace fallback off.
pub(super) const WORKSPACE_FALLBACK_ENV: &str = "NETSUKE_WHICH_WORKSPACE";

/// Decide whether the workspace fallback is enabled, reading through `read_env`.
///
/// This is the pure decision function; it owns no composition root. The
/// process-backed value is captured by `EnvSnapshot::capture` alongside the
/// resolver's other ambient reads, and the decision is derived from the
/// snapshot. Keeping the seam pure means the three outcomes — an explicit
/// value, an absent variable, and a non-UTF-8 value — can each be exercised
/// without mutating the process environment. The non-UTF-8 branch is
/// otherwise unreachable from a test, because constructing such a value
/// requires platform-specific `OsString` surgery on the live environment.
///
/// # Examples
///
/// ```rust,ignore
/// // A disabling value switches the fallback off, case-insensitively.
/// assert!(!workspace_fallback_enabled_with(|_| Ok("OFF".to_owned())));
/// // The switch is opt-out, so an unset variable leaves it enabled.
/// assert!(workspace_fallback_enabled_with(|_| Err(env::VarError::NotPresent)));
/// ```
pub(super) fn workspace_fallback_enabled_with<F>(read_env: F) -> bool
where
    F: FnOnce(&str) -> Result<String, env::VarError>,
{
    match read_env(WORKSPACE_FALLBACK_ENV) {
        Ok(value) => {
            let normalised = value.to_ascii_lowercase();
            !matches!(normalised.as_str(), "0" | "false" | "off")
        }
        Err(env::VarError::NotPresent) => true,
        Err(env::VarError::NotUnicode(_)) => {
            tracing::warn!(
                env = WORKSPACE_FALLBACK_ENV,
                "workspace fallback disabled because env var is not valid UTF-8",
            );
            false
        }
    }
}

#[cfg(test)]
#[path = "lookup/workspace/fallback_tests.rs"]
mod fallback_tests;
