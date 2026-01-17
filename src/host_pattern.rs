//! Shared host pattern validation helpers.
//!
//! The module centralises normalisation and matching logic so CLI parsing and
//! runtime policy evaluation agree on allowable host syntax.

use crate::localization::{self, LocalizedMessage, keys};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

#[derive(Copy, Clone)]
struct HostPatternInput<'a>(&'a str);

impl<'a> HostPatternInput<'a> {
    const fn as_str(self) -> &'a str {
        self.0
    }
}

#[derive(Copy, Clone)]
pub(crate) struct HostCandidate<'a>(pub(crate) &'a str);

impl<'a> HostCandidate<'a> {
    const fn as_str(self) -> &'a str {
        self.0
    }
}

struct ValidationContext<'a> {
    original: HostPatternInput<'a>,
}

impl<'a> ValidationContext<'a> {
    const fn new(original: HostPatternInput<'a>) -> Self {
        Self { original }
    }

    fn validate_label(&self, label: &str) -> Result<(), HostPatternError> {
        let original = self.original.as_str();
        if label.is_empty() {
            return Err(HostPatternError::EmptyLabel {
                pattern: original.to_owned(),
                message: localization::message(keys::HOST_PATTERN_EMPTY_LABEL)
                    .with_arg("pattern", original),
            });
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(HostPatternError::InvalidCharacters {
                pattern: original.to_owned(),
                message: localization::message(keys::HOST_PATTERN_INVALID_CHARS)
                    .with_arg("pattern", original),
            });
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(HostPatternError::InvalidLabelEdge {
                pattern: original.to_owned(),
                message: localization::message(keys::HOST_PATTERN_INVALID_LABEL_EDGE)
                    .with_arg("pattern", original),
            });
        }
        if label.len() > 63 {
            return Err(HostPatternError::LabelTooLong {
                pattern: original.to_owned(),
                message: localization::message(keys::HOST_PATTERN_LABEL_TOO_LONG)
                    .with_arg("pattern", original),
            });
        }
        Ok(())
    }
}

/// Errors emitted when parsing host allowlist/blocklist patterns.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HostPatternError {
    /// Input was empty or whitespace.
    #[error("{message}")]
    Empty {
        /// Localised error message.
        message: LocalizedMessage,
    },
    /// The pattern erroneously included a URL scheme.
    #[error("{message}")]
    ContainsScheme {
        /// Original host pattern string.
        pattern: String,
        /// Localised error message.
        message: LocalizedMessage,
    },
    /// The pattern contained path delimiters.
    #[error("{message}")]
    ContainsSlash {
        /// Original host pattern string.
        pattern: String,
        /// Localised error message.
        message: LocalizedMessage,
    },
    /// Wildcard patterns must include a suffix after `*.`.
    #[error("{message}")]
    MissingSuffix {
        /// Original host pattern string.
        pattern: String,
        /// Localised error message.
        message: LocalizedMessage,
    },
    /// Patterns may not contain empty labels between dots.
    #[error("{message}")]
    EmptyLabel {
        /// Original host pattern string.
        pattern: String,
        /// Localised error message.
        message: LocalizedMessage,
    },
    /// Patterns must only contain alphanumeric characters or `-`.
    #[error("{message}")]
    InvalidCharacters {
        /// Original host pattern string.
        pattern: String,
        /// Localised error message.
        message: LocalizedMessage,
    },
    /// Labels must not begin or end with a hyphen.
    #[error("{message}")]
    InvalidLabelEdge {
        /// Original host pattern string.
        pattern: String,
        /// Localised error message.
        message: LocalizedMessage,
    },
    /// Individual labels may not exceed 63 characters.
    #[error("{message}")]
    LabelTooLong {
        /// Original host pattern string.
        pattern: String,
        /// Localised error message.
        message: LocalizedMessage,
    },
    /// The full host (including dots) may not exceed 255 characters.
    #[error("{message}")]
    HostTooLong {
        /// Original host pattern string.
        pattern: String,
        /// Localised error message.
        message: LocalizedMessage,
    },
}

fn normalise_host_pattern(input: HostPatternInput<'_>) -> Result<(String, bool), HostPatternError> {
    let trimmed = input.as_str().trim();
    if trimmed.is_empty() {
        return Err(HostPatternError::Empty {
            message: localization::message(keys::HOST_PATTERN_EMPTY),
        });
    }
    if trimmed.contains("://") {
        return Err(HostPatternError::ContainsScheme {
            pattern: trimmed.to_owned(),
            message: localization::message(keys::HOST_PATTERN_CONTAINS_SCHEME)
                .with_arg("pattern", trimmed),
        });
    }
    if trimmed.contains('/') {
        return Err(HostPatternError::ContainsSlash {
            pattern: trimmed.to_owned(),
            message: localization::message(keys::HOST_PATTERN_CONTAINS_SLASH)
                .with_arg("pattern", trimmed),
        });
    }

    let (wildcard, host_body) = if let Some(suffix) = trimmed.strip_prefix("*.") {
        if suffix.is_empty() {
            return Err(HostPatternError::MissingSuffix {
                pattern: trimmed.to_owned(),
                message: localization::message(keys::HOST_PATTERN_MISSING_SUFFIX)
                    .with_arg("pattern", trimmed),
            });
        }
        (true, suffix)
    } else {
        (false, trimmed)
    };

    let normalised = host_body.to_ascii_lowercase();
    let mut total_len = 0usize;
    let ctx = ValidationContext::new(HostPatternInput(trimmed));
    for (index, label) in normalised.split('.').enumerate() {
        ctx.validate_label(label)?;
        total_len += label.len() + usize::from(index > 0);
    }
    if total_len > 255 {
        return Err(HostPatternError::HostTooLong {
            pattern: trimmed.to_owned(),
            message: localization::message(keys::HOST_PATTERN_TOO_LONG)
                .with_arg("pattern", trimmed),
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
        let (normalised, wildcard) = normalise_host_pattern(HostPatternInput(pattern))?;
        Ok(Self {
            pattern: normalised,
            wildcard,
        })
    }

    pub(crate) fn matches(&self, candidate: HostCandidate<'_>) -> bool {
        let host = candidate.as_str().to_ascii_lowercase();
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
        if self.wildcard {
            let text = format!("*.{}", self.pattern);
            serializer.serialize_str(&text)
        } else {
            serializer.serialize_str(&self.pattern)
        }
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
            parsed.matches(HostCandidate(host)) == expected,
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
            message.contains("Host pattern"),
            "error message should mention host pattern validation: {message}"
        );
    }
}
