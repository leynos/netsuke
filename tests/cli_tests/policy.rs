//! Network policy tests derived from CLI input.

use anyhow::{Context, Result, bail, ensure};
use netsuke::cli::Cli;
use netsuke::host_pattern::HostPattern;
use netsuke::stdlib::NetworkPolicyViolation;
use rstest::rstest;
use url::Url;

#[rstest]
fn cli_network_policy_defaults_to_https() -> Result<()> {
    let cli = Cli::default();
    let policy = cli.network_policy()?;
    let https = Url::parse("https://example.com").expect("parse https URL");
    let http = Url::parse("http://example.com").expect("parse http URL");
    ensure!(
        policy.evaluate(&https).is_ok(),
        "HTTPS should be permitted by default",
    );
    let err = policy
        .evaluate(&http)
        .expect_err("HTTP should be rejected by default");
    match err {
        NetworkPolicyViolation::SchemeNotAllowed { scheme } => {
            ensure!(scheme == "http", "unexpected scheme {scheme}");
        }
        other => bail!("expected scheme violation, got {other:?}"),
    }
    Ok(())
}

#[rstest]
fn cli_network_policy_default_deny_blocks_unknown_hosts() -> Result<()> {
    let mut cli = Cli {
        fetch_default_deny: true,
        ..Cli::default()
    };
    cli.fetch_allow_host
        .push(HostPattern::parse("example.com").context("parse allow host pattern")?);
    let policy = cli.network_policy()?;
    let allowed = Url::parse("https://example.com").expect("parse allowed URL");
    let denied = Url::parse("https://unauthorised.test").expect("parse denied URL");
    ensure!(
        policy.evaluate(&allowed).is_ok(),
        "explicit allowlist should permit matching host",
    );
    let err = policy
        .evaluate(&denied)
        .expect_err("default deny should block other hosts");
    match err {
        NetworkPolicyViolation::HostNotAllowlisted { host } => {
            ensure!(host == "unauthorised.test", "unexpected host {host}");
        }
        other => bail!("expected allowlist violation, got {other:?}"),
    }
    Ok(())
}

#[rstest]
fn cli_network_policy_blocklist_overrides_allowlist() -> Result<()> {
    let mut cli = Cli::default();
    cli.fetch_allow_host
        .push(HostPattern::parse("example.com").context("parse allow host pattern")?);
    cli.fetch_block_host
        .push(HostPattern::parse("example.com").context("parse block host pattern")?);
    let policy = cli.network_policy()?;
    let url = Url::parse("https://example.com").expect("parse conflicting URL");
    let err = policy
        .evaluate(&url)
        .expect_err("blocklist should override allowlist");
    let err_text = err.to_string();
    match err {
        NetworkPolicyViolation::HostBlocked { host } => {
            ensure!(host == "example.com", "unexpected host {host}");
            ensure!(
                err_text == "host 'example.com' is blocked",
                "unexpected error text: {err_text}",
            );
        }
        other => bail!("expected blocklist violation, got {other:?}"),
    }
    Ok(())
}

#[rstest]
fn cli_network_policy_rejects_invalid_scheme() {
    let mut cli = Cli::default();
    cli.fetch_allow_scheme.push(String::from("1http"));
    let err = cli
        .network_policy()
        .expect_err("invalid scheme should be rejected");
    assert!(
        err.to_string().contains("invalid characters"),
        "unexpected error text: {err}",
    );
}
