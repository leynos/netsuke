//! Shared host pattern validation helpers.
//!
//! The module centralises normalisation and matching logic so CLI parsing and
//! runtime policy evaluation agree on allowable host syntax.

use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

fn validate_label(label: &str, original: &str) -> Result<(), HostPatternError> {
    if label.is_empty() {
        return Err(HostPatternError::EmptyLabel {
            pattern: original.to_owned(),
        });
    }
    if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(HostPatternError::InvalidCharacters {
            pattern: original.to_owned(),
        });
    }
    if label.starts_with('-') || label.ends_with('-') {
        return Err(HostPatternError::InvalidLabelEdge {
            pattern: original.to_owned(),
        });
    }
    if label.len() > 63 {
        return Err(HostPatternError::LabelTooLong {
            pattern: original.to_owned(),
        });
    }
    Ok(())
}

/// Errors emitted when parsing host allowlist/blocklist patterns.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HostPatternError {
    /// Input was empty or whitespace.
    #[error("host pattern must not be empty")]
    Empty,
    /// The pattern erroneously included a URL scheme.
    #[error("host pattern '{pattern}' must not include a scheme")]
    ContainsScheme {
        /// Original host pattern string.
        pattern: String,
    },
    /// The pattern contained path delimiters.
    #[error("host pattern '{pattern}' must not contain '/'")]
    ContainsSlash {
        /// Original host pattern string.
        pattern: String,
    },
    /// Wildcard patterns must include a suffix after `*.`.
    #[error("wildcard host pattern '{pattern}' must include a suffix")]
    MissingSuffix {
        /// Original host pattern string.
        pattern: String,
    },
    /// Patterns may not contain empty labels between dots.
    #[error("host pattern '{pattern}' must not contain empty labels")]
    EmptyLabel {
        /// Original host pattern string.
        pattern: String,
    },
    /// Patterns must only contain alphanumeric characters or `-`.
    #[error("host pattern '{pattern}' contains invalid characters")]
    InvalidCharacters {
        /// Original host pattern string.
        pattern: String,
    },
    /// Labels must not begin or end with a hyphen.
    #[error("host pattern '{pattern}' must not start or end labels with '-'")]
    InvalidLabelEdge {
        /// Original host pattern string.
        pattern: String,
    },
    /// Individual labels may not exceed 63 characters.
    #[error("host pattern '{pattern}' must not contain labels longer than 63 characters")]
    LabelTooLong {
        /// Original host pattern string.
        pattern: String,
    },
    /// The full host (including dots) may not exceed 255 characters.
    #[error("host pattern '{pattern}' must not exceed 255 characters in total")]
    HostTooLong {
        /// Original host pattern string.
        pattern: String,
    },
}

pub(crate) fn normalise_host_pattern(pattern: &str) -> Result<(String, bool), HostPatternError> {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return Err(HostPatternError::Empty);
    }
    if trimmed.contains("://") {
        return Err(HostPatternError::ContainsScheme {
            pattern: trimmed.to_owned(),
        });
    }
    if trimmed.contains('/') {
        return Err(HostPatternError::ContainsSlash {
            pattern: trimmed.to_owned(),
        });
    }

    let (wildcard, host_body) = if let Some(suffix) = trimmed.strip_prefix("*.") {
        if suffix.is_empty() {
            return Err(HostPatternError::MissingSuffix {
                pattern: trimmed.to_owned(),
            });
        }
        (true, suffix)
    } else {
        (false, trimmed)
    };

    let normalised = host_body.to_ascii_lowercase();
    let mut total_len = 0usize;
    for (index, label) in normalised.split('.').enumerate() {
        validate_label(label, trimmed)?;
        total_len += label.len() + usize::from(index > 0);
    }
    if total_len > 255 {
        return Err(HostPatternError::HostTooLong {
            pattern: trimmed.to_owned(),
        });
    }

    Ok((normalised, wildcard))
}

/// Canonical host pattern storing the normalised body and wildcard flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPattern {
    pub(crate) pattern: String,
    pub(crate) wildcard: bool,
}

impl HostPattern {
    /// Parse a host pattern into its canonical representation.
    ///
    /// # Errors
    ///
    /// Returns an error when the pattern is empty, includes invalid
    /// characters, or uses a wildcard without a suffix.
    pub fn parse(pattern: &str) -> Result<Self, HostPatternError> {
        let (normalised, wildcard) = normalise_host_pattern(pattern)?;
        Ok(Self {
            pattern: normalised,
            wildcard,
        })
    }

    pub(crate) fn matches(&self, candidate: &str) -> bool {
        let host = candidate.to_ascii_lowercase();
        if self.wildcard {
            // Wildcard patterns match only subdomains, not the apex domain.
            // Example: "*.example.com" matches "sub.example.com" but not
            // "example.com".
            host.strip_suffix(&self.pattern)
                .and_then(|prefix| prefix.strip_suffix('.'))
                .is_some_and(|prefix| !prefix.is_empty())
        } else {
            host == self.pattern
        }
    }
}

impl<'a> TryFrom<&'a str> for HostPattern {
    type Error = HostPatternError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for HostPattern {
    type Error = HostPatternError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl FromStr for HostPattern {
    type Err = HostPatternError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for HostPattern {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let text = if self.wildcard {
            format!("*.{}", self.pattern)
        } else {
            self.pattern.clone()
        };
        serializer.serialize_str(&text)
    }
}

impl<'de> Deserialize<'de> for HostPattern {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
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
}
