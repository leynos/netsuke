//! Exact-name access policy for the manifest `env()` helper.
//!
//! The policy is a pure manifest-domain value: configuration adapters build it
//! at the composition root and template rendering evaluates it before any
//! environment reader can disclose a process value.

use std::collections::BTreeSet;

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
    pub fn allow_var(self, name: impl Into<String>) -> Self {
        self.allow_vars([name])
    }

    /// Append exact variable names to the allowlist.
    #[must_use]
    pub fn allow_vars<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        insert_names(&mut self.allowed_vars, names);
        self
    }

    /// Append one exact variable name to the blocklist.
    #[must_use]
    pub fn block_var(self, name: impl Into<String>) -> Self {
        self.block_vars([name])
    }

    /// Append exact variable names to the blocklist.
    #[must_use]
    pub fn block_vars<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        insert_names(&mut self.blocked_vars, names);
        self
    }

    /// Return whether a block rule or active allowlist denies `name`.
    fn name_is_blocked(&self, name: &str) -> bool {
        let normalized_name = normalize_name(name);
        if self.blocked_vars.contains(&normalized_name) {
            return true;
        }

        self.active_allowlist_rejects(&normalized_name)
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
            return Err(EnvPolicyViolation::Blocked);
        }

        Ok(())
    }
}

/// Reasons an environment-access policy rejected an `env()` lookup.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnvPolicyViolation {
    /// The variable name is blocked by a block rule or an active allowlist.
    #[error("environment access is blocked")]
    Blocked,
}

/// Normalize a variable name to the platform's environment lookup semantics.
///
/// Windows resolves environment variable names without regard to case. Other
/// supported platforms preserve case, so their policy matching remains exact.
fn normalize_name(name: &str) -> String {
    if cfg!(windows) {
        name.to_uppercase()
    } else {
        name.to_owned()
    }
}

/// Insert normalized variable names into one policy collection.
fn insert_names<I, S>(names_to_insert: &mut BTreeSet<String>, names: I)
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    names_to_insert.extend(names.into_iter().map(|name| normalize_name(&name.into())));
}

#[cfg(test)]
mod tests;
