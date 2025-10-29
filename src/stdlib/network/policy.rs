//! NOTE: This module intentionally exceeds the 400-line guideline because it
//! integrates the public API documentation and embedded tests, so it should
//! remain a single file.
//! Network policy enforcement for outbound requests made by `fetch`.
//!
//! The policy keeps URL scheme and host validation separate from the I/O code
//! so tests can exercise the decision logic without talking to the network.

use std::collections::BTreeSet;

use thiserror::Error;
use url::Url;

use crate::host_pattern::{HostPattern, HostPatternError};

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
    /// [`Self::deny_all_hosts`] switches to default-deny mode and activates the
    /// allowlist assembled via this method. Invoking `allow_hosts` beforehand
    /// simply stages the patterns so the later default-deny transition applies
    /// them immediately.
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
            .any(|pattern| pattern.matches(host))
        {
            return Err(NetworkPolicyViolation::HostBlocked {
                host: host.to_owned(),
            });
        }

        if self
            .allowed_hosts
            .as_ref()
            .is_some_and(|allowlist| !allowlist.iter().any(|pattern| pattern.matches(host)))
        {
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
mod tests {
    use super::*;

    use crate::host_pattern::HostPattern;
    use anyhow::{Result, ensure};
    use rstest::rstest;

    #[rstest]
    #[case("example.com", false)]
    #[case("*.example.com", true)]
    fn host_pattern_parse_detects_wildcard(
        #[case] pattern: &str,
        #[case] wildcard: bool,
    ) -> Result<()> {
        let parsed = HostPattern::parse(pattern)?;
        ensure!(
            parsed.wildcard == wildcard,
            "expected wildcard {wildcard} for pattern {pattern}",
        );
        Ok(())
    }

    #[rstest]
    #[case("example.com", "example.com", true)]
    #[case("example.com", "sub.example.com", false)]
    #[case("*.example.com", "sub.example.com", true)]
    #[case("*.example.com", "example.com", false)]
    #[case("*.example.com", "deep.sub.example.com", true)]
    #[case("*.example.com", "other.com", false)]
    fn host_pattern_matches_expected(
        #[case] pattern: &str,
        #[case] host: &str,
        #[case] expected: bool,
    ) -> Result<()> {
        let parsed = HostPattern::parse(pattern)?;
        ensure!(
            parsed.matches(host) == expected,
            "expected match={expected} for {host} against {pattern}",
        );
        Ok(())
    }

    #[rstest]
    #[case("-example.com")]
    #[case("example-.com")]
    #[case("exa mple.com")]
    #[case("*.bad-.test")]
    fn host_pattern_rejects_invalid_shapes(#[case] pattern: &str) {
        let err = HostPattern::parse(pattern).expect_err("invalid pattern should fail");
        let message = err.to_string();
        assert!(
            message.contains("host pattern"),
            "error message should mention host pattern validation: {message}"
        );
    }

    #[rstest]
    fn evaluate_rejects_missing_host() {
        let policy = NetworkPolicy::default()
            .allow_scheme("data")
            .expect("add data scheme");
        let url = Url::parse("data:text/plain,hello").expect("parse url");
        assert!(
            url.host_str().is_none(),
            "data URLs must not expose a host: {url:?}"
        );
        let err = policy.evaluate(&url).expect_err("missing host should fail");
        assert_eq!(err, NetworkPolicyViolation::MissingHost);
    }

    #[rstest]
    fn evaluate_rejects_disallowed_scheme() {
        let policy = NetworkPolicy::default();
        let url = Url::parse("http://example.com").expect("parse url");
        let err = policy
            .evaluate(&url)
            .expect_err("http scheme should be denied by default");
        assert_eq!(
            err,
            NetworkPolicyViolation::SchemeNotAllowed {
                scheme: String::from("http"),
            }
        );
    }

    #[rstest]
    fn evaluate_respects_allowlist() -> Result<()> {
        let policy = NetworkPolicy::default()
            .deny_all_hosts()
            .allow_hosts(["example.com", "*.example.org"])?;
        let allowed = Url::parse("https://example.com")?;
        let allowed_sub = Url::parse("https://www.example.org")?;
        let denied = Url::parse("https://unauthorised.test")?;
        policy.evaluate(&allowed).expect("allow exact host");
        policy.evaluate(&allowed_sub).expect("allow wildcard host");
        let err = policy
            .evaluate(&denied)
            .expect_err("non-allowlisted host should be blocked");
        ensure!(
            err == NetworkPolicyViolation::HostNotAllowlisted {
                host: String::from("unauthorised.test"),
            },
            "expected host to be unauthorised.test but was {err:?}",
        );
        Ok(())
    }

    #[rstest]
    fn evaluate_prefers_blocklist() -> Result<()> {
        let policy = NetworkPolicy::default()
            .allow_hosts(["example.com"])?
            .block_host("example.com")?;
        let url = Url::parse("https://example.com")?;
        let err = policy
            .evaluate(&url)
            .expect_err("blocklist should win when host appears in both lists");
        ensure!(
            err == NetworkPolicyViolation::HostBlocked {
                host: String::from("example.com"),
            },
            "expected blocklist to reject example.com but was {err:?}",
        );
        Ok(())
    }
}
