//! Network policy enforcement for outbound requests made by `fetch`.
//!
//! The policy keeps URL scheme and host validation separate from the I/O code
//! so tests can exercise the decision logic without talking to the network.

use std::collections::BTreeSet;

use anyhow::{bail, ensure};
use thiserror::Error;
use url::Url;

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
/// let url = Url::parse("https://example.com/data.txt").unwrap();
/// assert!(policy.evaluate(&url).is_ok());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkPolicy {
    allowed_schemes: BTreeSet<String>,
    allowed_hosts: Option<Vec<HostPattern>>,
    blocked_hosts: Vec<HostPattern>,
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
    /// let allowed = Url::parse("https://example.com").unwrap();
    /// let denied = Url::parse("http://example.com").unwrap();
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
    /// let url = Url::parse("http://localhost").unwrap();
    /// assert!(policy.evaluate(&url).is_ok());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the scheme is empty or contains invalid
    /// characters.
    pub fn allow_scheme(mut self, scheme: impl AsRef<str>) -> anyhow::Result<Self> {
        let candidate = scheme.as_ref();
        ensure!(!candidate.is_empty(), "scheme must not be empty");
        ensure!(
            candidate
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')),
            "scheme '{candidate}' contains invalid characters",
        );
        self.allowed_schemes.insert(candidate.to_ascii_lowercase());
        Ok(self)
    }

    /// Restrict requests to hosts that match the supplied patterns.
    ///
    /// Host patterns accept either exact hostnames or `*.example.com`
    /// wildcards. Subsequent calls append to the allowlist.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::stdlib::NetworkPolicy;
    /// use url::Url;
    ///
    /// let policy = NetworkPolicy::default()
    ///     .allow_hosts(["example.com", "*.example.org"])
    ///     .unwrap();
    /// let allowed = Url::parse("https://sub.example.org").unwrap();
    /// let denied = Url::parse("https://unauthorised.test").unwrap();
    /// assert!(policy.evaluate(&allowed).is_ok());
    /// assert!(policy.evaluate(&denied).is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when any host pattern is invalid or when the resulting
    /// allowlist would be empty.
    pub fn allow_hosts<I, S>(mut self, hosts: I) -> anyhow::Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut patterns = self.allowed_hosts.take().unwrap_or_default();
        for host in hosts {
            patterns.push(HostPattern::parse(host.as_ref())?);
        }
        if patterns.is_empty() {
            bail!("host allowlist must contain at least one entry");
        }
        self.allowed_hosts = Some(patterns);
        Ok(self)
    }

    /// Block every host until an allowlist is provided.
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
    ///     .unwrap();
    /// let allowed = Url::parse("https://example.com").unwrap();
    /// let denied = Url::parse("https://other.test").unwrap();
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
    ///     .unwrap();
    /// let denied = Url::parse("https://169.254.169.254").unwrap();
    /// assert!(policy.evaluate(&denied).is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the host pattern is invalid.
    pub fn block_host(mut self, host: impl AsRef<str>) -> anyhow::Result<Self> {
        let pattern = HostPattern::parse(host.as_ref())?;
        self.blocked_hosts.push(pattern);
        Ok(self)
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
    /// let valid = Url::parse("https://example.com").unwrap();
    /// let invalid = Url::parse("http://example.com").unwrap();
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
            .allowed_hosts
            .as_ref()
            .is_some_and(|allowlist| !allowlist.iter().any(|pattern| pattern.matches(host)))
        {
            return Err(NetworkPolicyViolation::HostNotAllowlisted {
                host: host.to_owned(),
            });
        }

        if self
            .blocked_hosts
            .iter()
            .any(|pattern| pattern.matches(host))
        {
            return Err(NetworkPolicyViolation::HostBlocked {
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostPattern {
    pattern: String,
    wildcard: bool,
}

impl HostPattern {
    fn parse(pattern: &str) -> anyhow::Result<Self> {
        let trimmed = pattern.trim();
        ensure!(!trimmed.is_empty(), "host pattern must not be empty");
        ensure!(
            !trimmed.contains("://"),
            "host pattern '{trimmed}' must not include a scheme",
        );
        ensure!(
            !trimmed.contains('/'),
            "host pattern '{trimmed}' must not contain '/'",
        );
        let (wildcard, normalised) = if let Some(suffix) = trimmed.strip_prefix("*.") {
            ensure!(
                !suffix.is_empty(),
                "wildcard host pattern '{trimmed}' must include a suffix",
            );
            (true, suffix.to_ascii_lowercase())
        } else {
            (false, trimmed.to_ascii_lowercase())
        };
        Ok(Self {
            pattern: normalised,
            wildcard,
        })
    }

    fn matches(&self, candidate: &str) -> bool {
        let host = candidate.to_ascii_lowercase();
        if self.wildcard {
            if host == self.pattern {
                return true;
            }
            host.strip_suffix(&self.pattern)
                .is_some_and(|prefix| prefix.ends_with('.') && prefix.len() > 1)
        } else {
            host == self.pattern
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    #[case("*.example.com", "example.com", true)]
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
