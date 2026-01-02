//! Network policy enforcement for outbound requests made by `fetch`.
//!
//! The policy keeps URL scheme and host validation separate from the I/O code
//! so tests can exercise the decision logic without talking to the network.

use std::collections::BTreeSet;

use thiserror::Error;
use url::Url;

use crate::host_pattern::{HostCandidate, HostPattern, HostPatternError};

/// Declarative allow- and deny-list policy for outbound network requests.
///
/// The policy validates URL schemes and hostnames before a request is
/// dispatched. By default only HTTPS requests are permitted; callers can
/// extend this to support other schemes or restrict hosts.
///
/// # Examples
///
/// ```rust
/// use netsuke::stdlib::NetworkPolicy;
/// use url::Url;
///
/// let policy = NetworkPolicy::default();
/// let url = Url::parse("https://example.com/data.txt").expect("parse URL");
/// assert!(policy.evaluate(&url).is_ok());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkPolicy {
    allowed_schemes: BTreeSet<String>,
    allowed_hosts: Option<Vec<HostPattern>>,
    blocked_hosts: Vec<HostPattern>,
}

/// Reasons a network policy configuration failed to build.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NetworkPolicyConfigError {
    /// The supplied scheme was empty.
    #[error("scheme must not be empty")]
    EmptyScheme,
    /// The supplied scheme contained invalid characters.
    #[error("scheme '{scheme}' contains invalid characters")]
    InvalidScheme {
        /// The rejected scheme string.
        scheme: String,
    },
    /// Attempted to enable default-deny without providing any allowlist entries.
    #[error("host allowlist must contain at least one entry")]
    EmptyAllowlist,
    /// Host pattern parsing failed.
    #[error(transparent)]
    HostPattern(#[from] HostPatternError),
}

impl NetworkPolicy {
    /// Create a policy that allows only HTTPS requests.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::stdlib::NetworkPolicy;
    /// use url::Url;
    ///
    /// let policy = NetworkPolicy::https_only();
    /// let allowed = Url::parse("https://example.com").expect("parse URL");
    /// let denied = Url::parse("http://example.com").expect("parse URL");
    /// assert!(policy.evaluate(&allowed).is_ok());
    /// assert!(policy.evaluate(&denied).is_err());
    /// ```
    #[must_use]
    pub fn https_only() -> Self {
        let mut schemes = BTreeSet::new();
        schemes.insert(String::from("https"));
        Self {
            allowed_schemes: schemes,
            allowed_hosts: None,
            blocked_hosts: Vec::new(),
        }
    }

    /// Permit an additional URL scheme such as `http`.
    ///
    /// The scheme is lower-cased internally. Invalid scheme characters cause an
    /// error, allowing CLI validation to surface clear diagnostics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::stdlib::NetworkPolicy;
    /// use url::Url;
    ///
    /// let policy = NetworkPolicy::default()
    ///     .allow_scheme("http")
    ///     .expect("add http scheme");
    /// let url = Url::parse("http://localhost").expect("parse URL");
    /// assert!(policy.evaluate(&url).is_ok());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the scheme is empty or contains invalid
    /// characters.
    pub fn allow_scheme(
        mut self,
        scheme: impl AsRef<str>,
    ) -> Result<Self, NetworkPolicyConfigError> {
        let candidate = scheme.as_ref();
        if candidate.is_empty() {
            return Err(NetworkPolicyConfigError::EmptyScheme);
        }
        let mut chars = candidate.chars();
        if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
            return Err(NetworkPolicyConfigError::InvalidScheme {
                scheme: candidate.to_owned(),
            });
        }
        if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
            return Err(NetworkPolicyConfigError::InvalidScheme {
                scheme: candidate.to_owned(),
            });
        }
        self.allowed_schemes.insert(candidate.to_ascii_lowercase());
        Ok(self)
    }

    fn extend_allowed_hosts<I>(mut self, patterns_iter: I) -> Result<Self, NetworkPolicyConfigError>
    where
        I: IntoIterator<Item = HostPattern>,
    {
        let mut patterns = self.allowed_hosts.take().unwrap_or_default();
        patterns.extend(patterns_iter);
        if patterns.is_empty() {
            return Err(NetworkPolicyConfigError::EmptyAllowlist);
        }
        self.allowed_hosts = Some(patterns);
        Ok(self)
    }

    /// Restrict requests to hosts that match the supplied patterns.
    ///
    /// Host patterns accept either exact hostnames or `*.example.com`
    /// wildcards. Subsequent calls append to the allowlist.
    ///
    /// By default the policy allows every host because
    /// [`Self::default`] leaves default-deny disabled. Calling
    /// [`Self::allow_hosts`] immediately activates the allowlist and causes any
    /// host not matched by the supplied patterns to be rejected. As an
    /// alternative, [`Self::deny_all_hosts`] enables default-deny mode with an
    /// empty allowlist so patterns can be added incrementally afterwards.
    ///
    /// Patterns must be ASCII (or punycode) to match the `url::Url`
    /// representation. Unicode domains should be converted to punycode
    /// before being passed to the policy.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::stdlib::NetworkPolicy;
    /// use url::Url;
    ///
    /// let policy = NetworkPolicy::default()
    ///     .deny_all_hosts()
    ///     .allow_hosts(["example.com", "*.example.org"])
    ///     .expect("configure host allowlist");
    /// let allowed = Url::parse("https://sub.example.org").expect("parse URL");
    /// let denied = Url::parse("https://unauthorised.test").expect("parse URL");
    /// assert!(policy.evaluate(&allowed).is_ok());
    /// assert!(policy.evaluate(&denied).is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when any host pattern is invalid or when the resulting
    /// allowlist would be empty.
    pub fn allow_hosts<I, S>(self, hosts: I) -> Result<Self, NetworkPolicyConfigError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let parsed = hosts
            .into_iter()
            .map(|host| HostPattern::parse(host.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        self.extend_allowed_hosts(parsed)
    }

    /// Append pre-parsed host patterns to the allowlist.
    ///
    /// This variant avoids reparsing patterns that were validated elsewhere
    /// (for example, by CLI flag parsers).
    ///
    /// # Errors
    ///
    /// Returns an error when the resulting allowlist would be empty.
    pub fn allow_host_patterns<I>(self, hosts: I) -> Result<Self, NetworkPolicyConfigError>
    where
        I: IntoIterator<Item = HostPattern>,
    {
        self.extend_allowed_hosts(hosts)
    }

    /// Block every host until an allowlist is provided.
    ///
    /// Calling this method enables default-deny mode immediately. Any patterns
    /// accumulated through [`Self::allow_hosts`] beforehand become active once
    /// default-deny is enabled, so callers may configure the allowlist in either
    /// order.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::stdlib::NetworkPolicy;
    /// use url::Url;
    ///
    /// let policy = NetworkPolicy::default()
    ///     .deny_all_hosts()
    ///     .allow_hosts(["example.com"])
    ///     .expect("configure host allowlist");
    /// let allowed = Url::parse("https://example.com").expect("parse URL");
    /// let denied = Url::parse("https://other.test").expect("parse URL");
    /// assert!(policy.evaluate(&allowed).is_ok());
    /// assert!(policy.evaluate(&denied).is_err());
    /// ```
    #[must_use]
    pub fn deny_all_hosts(mut self) -> Self {
        self.allowed_hosts = Some(Vec::new());
        self
    }

    /// Append a host pattern to the blocklist.
    ///
    /// Blocked hosts are denied even when the allowlist permits them.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::stdlib::NetworkPolicy;
    /// use url::Url;
    ///
    /// let policy = NetworkPolicy::default()
    ///     .block_host("169.254.169.254")
    ///     .expect("block metadata host");
    /// let denied = Url::parse("https://169.254.169.254").expect("parse URL");
    /// assert!(policy.evaluate(&denied).is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the host pattern is invalid.
    pub fn block_host(mut self, host: impl AsRef<str>) -> Result<Self, NetworkPolicyConfigError> {
        let pattern = HostPattern::parse(host.as_ref())?;
        self.blocked_hosts.push(pattern);
        Ok(self)
    }

    /// Append a pre-parsed host pattern to the blocklist without reparsing.
    #[must_use]
    pub fn block_host_pattern(mut self, host: HostPattern) -> Self {
        self.blocked_hosts.push(host);
        self
    }

    /// Validate the supplied URL against the configured policy.
    ///
    /// Returns `Ok(())` when the scheme and host are permitted and `Err`
    /// detailing the violated rule otherwise.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::stdlib::NetworkPolicy;
    /// use url::Url;
    ///
    /// let policy = NetworkPolicy::default();
    /// let valid = Url::parse("https://example.com").expect("parse URL");
    /// let invalid = Url::parse("http://example.com").expect("parse URL");
    /// assert!(policy.evaluate(&valid).is_ok());
    /// assert!(policy.evaluate(&invalid).is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the scheme is disallowed, the host is missing or
    /// not on the allowlist, or the host matches a blocklist entry.
    pub fn evaluate(&self, url: &Url) -> Result<(), NetworkPolicyViolation> {
        let scheme = url.scheme();
        if !self.allowed_schemes.contains(scheme) {
            return Err(NetworkPolicyViolation::SchemeNotAllowed {
                scheme: scheme.to_owned(),
            });
        }

        let host = url
            .host_str()
            .filter(|host| !host.is_empty())
            .ok_or(NetworkPolicyViolation::MissingHost)?;
        if self
            .blocked_hosts
            .iter()
            .any(|pattern| pattern.matches(HostCandidate(host)))
        {
            return Err(NetworkPolicyViolation::HostBlocked {
                host: host.to_owned(),
            });
        }

        if self.allowed_hosts.as_ref().is_some_and(|allowlist| {
            !allowlist
                .iter()
                .any(|pattern| pattern.matches(HostCandidate(host)))
        }) {
            return Err(NetworkPolicyViolation::HostNotAllowlisted {
                host: host.to_owned(),
            });
        }

        Ok(())
    }
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self::https_only()
    }
}

/// Detailed reasons for a network policy evaluation failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NetworkPolicyViolation {
    /// The URL scheme is not present in the allowlist.
    #[error("scheme '{scheme}' is not permitted")]
    SchemeNotAllowed {
        /// The scheme provided by the caller.
        scheme: String,
    },
    /// The URL does not contain a host portion.
    #[error("URL must include a host")]
    MissingHost,
    /// The host is absent from the allowlist when default deny is active.
    #[error("host '{host}' is not allowlisted")]
    HostNotAllowlisted {
        /// Hostname that is absent from the allowlist.
        host: String,
    },
    /// The host matches one of the configured blocklist rules.
    #[error("host '{host}' is blocked")]
    HostBlocked {
        /// Hostname that matched a blocklist entry.
        host: String,
    },
}

#[cfg(test)]
mod tests;
