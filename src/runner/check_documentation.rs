//! Published rule-reference URLs for `netsuke check` output.
//!
//! This runner adapter owns the repository publication location. The lint
//! domain retains only stable rule identifiers and presentation data, so it
//! stays independent of GitHub and any other documentation transport.

/// Published rule-reference document used by check output adapters.
const RULE_REFERENCE_URL: &str =
    "https://github.com/leynos/netsuke/blob/main/docs/netsuke-linter-rules.md";

/// Build the published rule-reference URL for `rule_name`.
#[must_use]
pub(super) fn rule_documentation_url(rule_name: &str) -> String {
    format!("{RULE_REFERENCE_URL}#{rule_name}")
}

#[cfg(test)]
#[path = "check_documentation_tests.rs"]
mod tests;
