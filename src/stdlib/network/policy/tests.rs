use super::*;

use anyhow::{Result, ensure};
use rstest::rstest;
use url::Url;

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

#[rstest]
fn evaluate_blocklist_precedence_with_allowlist_wildcard() -> Result<()> {
    let policy = NetworkPolicy::default()
        .allow_hosts(["*.example.com"])?
        .block_host("blocked.example.com")?;
    let url = Url::parse("https://blocked.example.com")?;
    let err = policy
        .evaluate(&url)
        .expect_err("blocklist should win when host matches both lists");
    ensure!(
        err == NetworkPolicyViolation::HostBlocked {
            host: String::from("blocked.example.com"),
        },
        "expected blocklist to reject blocked.example.com but was {err:?}",
    );
    Ok(())
}

#[rstest]
fn evaluate_blocklist_precedence_with_wildcard() -> Result<()> {
    let policy = NetworkPolicy::default()
        .allow_hosts(["sub.example.com"])?
        .block_host("*.example.com")?;
    let url = Url::parse("https://sub.example.com")?;
    let err = policy
        .evaluate(&url)
        .expect_err("blocklist should win when wildcard matches allowlisted host");
    ensure!(
        err == NetworkPolicyViolation::HostBlocked {
            host: String::from("sub.example.com"),
        },
        "expected blocklist to reject sub.example.com but was {err:?}",
    );
    Ok(())
}
