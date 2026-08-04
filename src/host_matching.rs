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
    const fn as_str(self) -> &'a str {
        self.0
    }
}

impl HostPattern {
    /// Return whether `candidate` is covered by this pattern.
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

#[cfg(test)]
mod tests {
    //! Unit tests for wildcard and exact host matching.
    use super::*;

    use anyhow::{Result, ensure};
    use rstest::rstest;

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
}
