//! Integration-style tests for the network policy, covering host allow/block
//! precedence and scheme validation behaviour.

use super::*;

use anyhow::{Result, ensure};
use rstest::rstest;
use url::Url;

struct BlocklistPrecedenceCase<'a> {
    allow_patterns: &'a [&'a str],
    block_pattern: &'a str,
    test_url: &'a str,
    expected_blocked_host: &'a str,
    failure_message: &'a str,
}

#[rstest]
#[case("http!")]
#[case("123abc")]
fn allow_scheme_rejects_invalid_input(#[case] scheme: &str) {
    let err = NetworkPolicy::default()
        .allow_scheme(scheme)
        .expect_err("invalid schemes should be rejected");
    assert!(
        matches!(
            err,
            NetworkPolicyConfigError::InvalidScheme { scheme: ref rejected }
                if rejected == scheme
        ),
        "expected InvalidScheme for '{scheme}', got {err:?}"
    );
}

#[rstest]
fn allow_hosts_appends_patterns() -> Result<()> {
    let policy = NetworkPolicy::default()
        .deny_all_hosts()
        .allow_hosts(["example.com"])?
        .allow_hosts(["*.example.org"])?;
    let exact = Url::parse("https://example.com")?;
    let wildcard = Url::parse("https://sub.example.org")?;
    policy
        .evaluate(&exact)
        .expect("exact host should remain allowed");
    policy
        .evaluate(&wildcard)
        .expect("wildcard host should be appended");
    Ok(())
}

#[rstest]
fn allow_hosts_activates_allowlist() -> Result<()> {
    let policy = NetworkPolicy::default().allow_hosts(["example.com"])?;
    let allowed = Url::parse("https://example.com")?;
    let denied = Url::parse("https://unauthorised.test")?;
    policy
        .evaluate(&allowed)
        .expect("allowlisted host should pass");
    let err = policy
        .evaluate(&denied)
        .expect_err("non-allowlisted host should be rejected");
    ensure!(
        err == NetworkPolicyViolation::HostNotAllowlisted {
            host: String::from("unauthorised.test"),
        },
        "expected host unauthorised.test but was {err:?}",
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
#[case(BlocklistPrecedenceCase {
    allow_patterns: &["example.com"],
    block_pattern: "example.com",
    test_url: "https://example.com",
    expected_blocked_host: "example.com",
    failure_message: "blocklist should win when host appears in both lists",
})]
#[case(BlocklistPrecedenceCase {
    allow_patterns: &["*.example.com"],
    block_pattern: "blocked.example.com",
    test_url: "https://blocked.example.com",
    expected_blocked_host: "blocked.example.com",
    failure_message: "blocklist should win when host matches allowlist wildcard",
})]
#[case(BlocklistPrecedenceCase {
    allow_patterns: &["sub.example.com"],
    block_pattern: "*.example.com",
    test_url: "https://sub.example.com",
    expected_blocked_host: "sub.example.com",
    failure_message: "blocklist should win when wildcard matches allowlisted host",
})]
fn evaluate_blocklist_precedence(#[case] case: BlocklistPrecedenceCase<'_>) -> Result<()> {
    let BlocklistPrecedenceCase {
        allow_patterns,
        block_pattern,
        test_url,
        expected_blocked_host,
        failure_message,
    } = case;
    let policy = NetworkPolicy::default()
        .allow_hosts(allow_patterns.iter().copied())?
        .block_host(block_pattern)?;
    let url = Url::parse(test_url)?;
    let err = policy.evaluate(&url).expect_err(failure_message);
    ensure!(
        err == NetworkPolicyViolation::HostBlocked {
            host: String::from(expected_blocked_host),
        },
        "expected blocklist to reject {expected_blocked_host} but was {err:?}",
    );
    Ok(())
}
