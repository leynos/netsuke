//! Matching of concrete hostnames against parsed host patterns.
//!
//! Pattern *syntax* lives in [`crate::host_pattern`]; the matching rules that
//! consume a parsed pattern live here. The split keeps
//! [`crate::host_pattern`] free of anything the Clap schema does not need, so
//! `build.rs` can compile it for man-page generation without recompiling
//! runtime policy evaluation. See the note in `build.rs`.

use crate::host_pattern::HostPattern;

/// A concrete hostname being tested against a [`HostPattern`].
#[derive(Copy, Clone)]
pub(crate) struct HostCandidate<'a>(pub(crate) &'a str);

impl<'a> HostCandidate<'a> {
    /// Return the wrapped hostname.
    const fn as_str(self) -> &'a str {
        self.0
    }
}

impl HostPattern {
    /// Return whether `candidate` is covered by this pattern.
    ///
    /// A terminal DNS dot is ignored before matching. Wildcards still require
    /// a non-empty subdomain prefix, so they do not match the apex.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let exact = HostPattern::parse("example.test")?;
    /// assert!(exact.matches(HostCandidate("example.test.")));
    ///
    /// let wildcard = HostPattern::parse("*.example.test")?;
    /// assert!(wildcard.matches(HostCandidate("sub.example.test.")));
    /// assert!(!wildcard.matches(HostCandidate("example.test.")));
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub(crate) fn matches(&self, candidate: HostCandidate<'_>) -> bool {
        let lowercased_host = candidate.as_str().to_ascii_lowercase();
        let host = lowercased_host
            .strip_suffix('.')
            .unwrap_or(&lowercased_host);
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

#[cfg(test)]
mod tests {
    //! Unit tests for wildcard and exact host matching.
    use super::*;

    use anyhow::{Result, ensure};
    use proptest::{prelude::*, test_runner::TestCaseError};
    use rstest::rstest;

    /// Generate one ASCII DNS-label-safe hostname component.
    fn ascii_dns_label_strategy() -> impl Strategy<Value = String> {
        "[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?"
    }

    /// Parse a generated valid pattern without losing proptest's shrinking context.
    fn parse_generated_pattern(pattern: &str) -> Result<HostPattern, TestCaseError> {
        HostPattern::parse(pattern).map_err(|error| TestCaseError::fail(error.to_string()))
    }

    /// Verify representative exact and wildcard host matches.
    #[rstest]
    #[case("example.com", "example.com", true)]
    #[case("example.com", "sub.example.com", false)]
    #[case("*.example.com", "sub.example.com", true)]
    #[case("*.example.com", "example.com", false)]
    #[case("*.example.com", "deep.sub.example.com", true)]
    #[case("*.example.com", "other.com", false)]
    #[case("example.com", "example.com.", true)]
    #[case("*.example.com", "sub.example.com.", true)]
    #[case("example.com", "", false)]
    #[case("example.com", "ÉXAMPLE.COM", false)]
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

    /// Verify candidate normalization changes ASCII letters only.
    #[test]
    fn host_matching_normalizes_ascii_candidates_only() -> Result<()> {
        let pattern = HostPattern::parse("example.test")?;

        ensure!(
            pattern.matches(HostCandidate("EXAMPLE.TEST")),
            "ASCII case variants should match"
        );
        ensure!(
            !pattern.matches(HostCandidate("ÉXAMPLE.TEST")),
            "to_ascii_lowercase should leave non-ASCII letters unchanged"
        );
        Ok(())
    }

    proptest! {
        /// Verify exact patterns match ASCII case variants.
        #[test]
        fn exact_patterns_match_ascii_case_insensitively(
            label in ascii_dns_label_strategy(),
        ) {
            let pattern = format!("{label}.example.test");
            let candidate = pattern.to_ascii_uppercase();
            let parsed = parse_generated_pattern(&pattern)?;

            prop_assert!(parsed.matches(HostCandidate(&candidate)));
        }

        /// Verify wildcard patterns match every non-empty subdomain prefix.
        #[test]
        fn wildcard_patterns_match_every_nonempty_ascii_subdomain_prefix(
            labels in prop::collection::vec(ascii_dns_label_strategy(), 1..5),
        ) {
            let prefix = labels.join(".");
            let candidate = format!("{prefix}.example.test").to_ascii_uppercase();
            let parsed = parse_generated_pattern("*.example.test")?;

            prop_assert!(parsed.matches(HostCandidate(&candidate)));
            prop_assert!(!parsed.matches(HostCandidate("example.test")));
        }

        /// Verify exact patterns reject strict suffixes and superdomains.
        #[test]
        fn exact_patterns_reject_strict_suffixes_and_superdomains(
            label in ascii_dns_label_strategy(),
        ) {
            let pattern = format!("{label}.example.test");
            let superdomain = format!("sub.{pattern}");
            let parsed = parse_generated_pattern(&pattern)?;

            prop_assert!(!parsed.matches(HostCandidate("example.test")));
            prop_assert!(!parsed.matches(HostCandidate(&superdomain)));
        }
    }
}
