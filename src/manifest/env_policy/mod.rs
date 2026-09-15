//! Exact-name access policy for the manifest `env()` helper.
//!
//! The policy is a pure manifest-domain value: configuration adapters build it
//! at the composition root and template rendering evaluates it before any
//! environment reader can disclose a process value.

use std::collections::BTreeSet;

use crate::localization::{self, LocalizedMessage, keys};

/// Declarative allow- and block-list policy for manifest environment access.
///
/// An empty allowlist is permissive for backwards compatibility. Adding an
/// allowlist entry enables default-deny, while blocklist entries always deny.
/// Variable names are exact strings; this policy deliberately has no pattern
/// syntax.
///
/// # Examples
///
/// ```rust
/// use netsuke::manifest::EnvAccessPolicy;
///
/// let policy = EnvAccessPolicy::default().allow_var("CI");
/// assert!(policy.evaluate("CI").is_ok());
/// assert!(policy.evaluate("SECRET").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnvAccessPolicy {
    /// Exact variable names permitted once default-deny is active.
    allowed_vars: BTreeSet<String>,
    /// Exact variable names denied regardless of the allowlist.
    blocked_vars: BTreeSet<String>,
}

impl EnvAccessPolicy {
    /// Append one exact variable name to the allowlist.
    #[must_use]
    pub fn allow_var(mut self, name: impl Into<String>) -> Self {
        self.allowed_vars.insert(name.into());
        self
    }

    /// Append exact variable names to the allowlist.
    #[must_use]
    pub fn allow_vars<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_vars.extend(names.into_iter().map(Into::into));
        self
    }

    /// Append one exact variable name to the blocklist.
    #[must_use]
    pub fn block_var(mut self, name: impl Into<String>) -> Self {
        self.blocked_vars.insert(name.into());
        self
    }

    /// Append exact variable names to the blocklist.
    #[must_use]
    pub fn block_vars<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.blocked_vars.extend(names.into_iter().map(Into::into));
        self
    }

    /// Return whether a block rule or active allowlist denies `name`.
    fn name_is_blocked(&self, name: &str) -> bool {
        if self.blocked_vars.contains(name) {
            return true;
        }

        self.active_allowlist_rejects(name)
    }

    /// Return whether an active allowlist excludes `name`.
    fn active_allowlist_rejects(&self, name: &str) -> bool {
        !self.allowed_vars.is_empty() && !self.allowed_vars.contains(name)
    }

    /// Validate one environment variable name against this policy.
    ///
    /// Block rules take precedence. A non-empty allowlist activates
    /// default-deny; an empty allowlist preserves the historical default-allow
    /// behaviour.
    ///
    /// # Errors
    ///
    /// Returns [`EnvPolicyViolation::Blocked`] when the name is blocked or is
    /// absent from an active allowlist.
    pub fn evaluate(&self, name: &str) -> Result<(), EnvPolicyViolation> {
        if self.name_is_blocked(name) {
            return Err(EnvPolicyViolation::Blocked {
                message: localization::message(keys::MANIFEST_ENV_BLOCKED),
            });
        }

        Ok(())
    }
}

/// Reasons an environment-access policy rejected an `env()` lookup.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnvPolicyViolation {
    /// The variable name is blocked by a block rule or an active allowlist.
    #[error("{message}")]
    Blocked {
        /// Fixed localized diagnostic that deliberately omits the variable name.
        message: LocalizedMessage,
    },
}

#[cfg(test)]
mod tests;
