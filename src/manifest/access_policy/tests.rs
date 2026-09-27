//! Tests for exact-name manifest environment access policy decisions.

use super::*;
use rstest::rstest;

/// Preserve default-allow behaviour until an allowlist is configured.
#[rstest]
#[case("CI")]
#[case("PACKAGE_REGISTRY_TOKEN")]
#[case("UNLISTED_VALUE")]
fn empty_lists_allow_every_name(#[case] name: &str) {
    assert!(
        EnvAccessPolicy::default().evaluate(name).is_ok(),
        "empty lists should preserve default-allow for {name}"
    );
}

/// Activate default-deny when the allowlist has at least one entry.
#[rstest]
fn allowlist_restricts_names() {
    let policy = EnvAccessPolicy::default().allow_var("CI");

    assert!(
        policy.evaluate("CI").is_ok(),
        "allowlisted name should pass"
    );
    assert!(
        matches!(
            policy.evaluate("UNLISTED"),
            Err(EnvPolicyViolation::Blocked)
        ),
        "an unlisted name must be blocked once default-deny is active"
    );
}

/// Block only configured names when no allowlist activates default-deny.
#[rstest]
fn blocklist_without_allowlist_blocks_only_matching_name() {
    let policy = EnvAccessPolicy::default().block_var("GITHUB_TOKEN");

    assert!(policy.evaluate("CI").is_ok(), "unblocked name should pass");
    assert!(
        matches!(
            policy.evaluate("GITHUB_TOKEN"),
            Err(EnvPolicyViolation::Blocked)
        ),
        "a blocklisted name must be blocked"
    );
}

/// Reject near-miss names that share a prefix with an allow entry.
///
/// A prefix or pattern matcher would pass every other case in this file, so
/// these near misses are what hold the exact-match rule in place.
#[rstest]
fn allowlist_rejects_near_miss_names() {
    let policy = EnvAccessPolicy::default().allow_var("CI");

    assert!(
        policy.evaluate("CI_EXTRA").is_err(),
        "a prefix-extended name must not inherit the allow entry"
    );
    assert!(
        policy.evaluate("C").is_err(),
        "a prefix-truncated name must not inherit the allow entry"
    );
}

/// Leave near-miss names unblocked while no allowlist is active.
///
/// Blocklist-only matching must stay exact in both directions: a wider block
/// would deny an unrelated credential-adjacent name the operator never named.
#[rstest]
fn blocklist_without_allowlist_exempts_near_miss_names() {
    let policy = EnvAccessPolicy::default().block_var("GITHUB_TOKEN");

    assert!(
        policy.evaluate("GITHUB_TOKEN_EXTRA").is_ok(),
        "a prefix-extended name must not inherit the block entry"
    );
    assert!(
        policy.evaluate("TOKEN").is_ok(),
        "a suffix of the blocked name must not inherit the block entry"
    );
}

/// Give an exact block rule precedence over an exact allow rule.
#[rstest]
fn blocklist_overrides_allowlist() {
    let policy = EnvAccessPolicy::default()
        .allow_vars(["CI", "GITHUB_TOKEN"])
        .block_var("GITHUB_TOKEN");

    assert!(
        policy.evaluate("CI").is_ok(),
        "other allowed names should pass"
    );
    assert!(
        matches!(
            policy.evaluate("GITHUB_TOKEN"),
            Err(EnvPolicyViolation::Blocked)
        ),
        "a matching block rule must override the allow rule"
    );
}

/// Match Windows environment-variable names without regard to case.
#[cfg(windows)]
#[rstest]
fn windows_case_insensitive_name_matching() {
    let allowed = EnvAccessPolicy::default().allow_var("PATH");
    assert!(
        allowed.evaluate("Path").is_ok(),
        "a Windows case variant should match the allowlist"
    );

    let blocked = allowed.block_var("PATH");
    assert!(
        matches!(blocked.evaluate("Path"), Err(EnvPolicyViolation::Blocked)),
        "a Windows case variant should match the blocklist"
    );
}
