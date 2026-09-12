//! Unit and property tests for the pure redirect-chain decisions.
//!
//! The properties below hold for every redirect chain regardless of the
//! locations a server chooses, so they complement the end-to-end tests that
//! pin one observable behaviour per fixture.

use anyhow::{Context, Result, ensure};
use proptest::prelude::*;
use rstest::{fixture, rstest};
use url::Url;

use super::*;

/// URL every generated chain starts from.
const INITIAL_URL: &str = "http://allowed.example/start";

/// Build a policy that permits HTTP on `hosts` and refuses every other host.
///
/// # Errors
///
/// Returns an error when `http` is not a valid scheme, or when `hosts` holds a
/// value that is not a valid allowlist pattern.
fn policy_for_hosts(hosts: &[&'static str]) -> Result<NetworkPolicy> {
    NetworkPolicy::default()
        .allow_scheme("http")
        .context("HTTP should be a valid scheme")?
        .deny_all_hosts()
        .allow_hosts(hosts.iter().copied())
        .context("generated hosts should be valid patterns")
}

/// Parse the base URL every generated chain starts from.
///
/// # Errors
///
/// Returns an error when [`INITIAL_URL`] is not a well-formed URL.
fn initial_url() -> Result<Url> {
    Url::parse(INITIAL_URL).context("initial URL should parse")
}

/// Generate bounded absolute and relative redirect locations.
fn generated_locations() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(
        prop_oneof![
            Just(String::from("/next")),
            Just(String::from("/other")),
            (0_u8..8).prop_map(|hop| format!("/hop/{hop}")),
            (0_u8..8).prop_map(|hop| format!("http://allowed.example/hop/{hop}")),
        ],
        1..=8,
    )
}

/// Generate absolute targets spread across allowed, blocked, and unknown hosts.
fn generated_targets() -> impl Strategy<Value = String> {
    (
        prop_oneof![Just("http"), Just("https")],
        prop_oneof![
            Just("allowed.example"),
            Just("blocked.example"),
            Just("other.example"),
            Just("127.0.0.1"),
        ],
        0_u8..4,
    )
        .prop_map(|(scheme, host, path)| format!("{scheme}://{host}/target/{path}"))
}

/// Base URL and matching policy every chain case below starts from.
struct ChainSetup {
    /// URL a chain starts at.
    base: Url,
    /// Policy that permits HTTP on `allowed.example` only.
    policy: NetworkPolicy,
}

/// Provide the initial URL and the allowlisted policy that accepts it.
///
/// Returns an error rather than panicking, so a malformed fixture is reported
/// by the case that needs it and not by the fixture itself.
#[fixture]
fn chain_setup() -> Result<ChainSetup> {
    Ok(ChainSetup {
        base: initial_url()?,
        policy: policy_for_hosts(&["allowed.example"])?,
    })
}

/// Verify an accepted chain never exceeds the hop limit, repeats a URL, or
/// misnumbers a hop, checked against an independent record of what was sent.
///
/// The locations below mix a repeated target, relative hops within one origin,
/// and an absolute hop, so a single chain exercises acceptance, loop refusal,
/// and the limit in the order a server would present them.
#[rstest]
fn bounded_chain_matches_an_independent_dispatch_record(
    chain_setup: Result<ChainSetup>,
) -> Result<()> {
    let ChainSetup { base, policy } = chain_setup?;
    let mut chain = RedirectChain::new(&base, &policy);
    let mut dispatched = vec![base];
    let mut accepted = 0_usize;
    let locations = [
        "/next",
        "/next",
        "/hop/2",
        "/hop/3",
        "/hop/4",
        "/hop/5",
        "/hop/6",
        "http://allowed.example/hop/7",
        "/hop/8",
    ];

    for location in locations {
        let Ok(transition) = chain.advance(Some(location)) else {
            continue;
        };
        accepted += 1;
        ensure!(
            accepted <= FETCH_REDIRECT_LIMIT,
            "a chain must accept at most {FETCH_REDIRECT_LIMIT} redirects, got {accepted}"
        );
        ensure!(
            transition.hop == accepted,
            "hop numbers must increase by one per accepted redirect: hop {} at accepted {accepted}",
            transition.hop,
        );
        ensure!(
            !dispatched.contains(&transition.next_url),
            "a chain must never request the same URL twice: {}",
            transition.next_url,
        );
        dispatched.push(transition.next_url);
    }

    ensure!(
        accepted == FETCH_REDIRECT_LIMIT,
        "the chain should accept exactly as many redirects as the limit allows, got {accepted}"
    );
    ensure!(
        accepted == chain.hops(),
        "the chain must count exactly the redirects it accepted: accepted {accepted}, counted {}",
        chain.hops(),
    );
    Ok(())
}

/// Verify a chain of distinct hops stops exactly at the configured limit.
#[rstest]
fn distinct_chain_stops_at_the_redirect_limit(chain_setup: Result<ChainSetup>) -> Result<()> {
    let ChainSetup { base, policy } = chain_setup?;
    let mut chain = RedirectChain::new(&base, &policy);

    for hop in 0..FETCH_REDIRECT_LIMIT {
        let location = format!("/hop/{hop}");
        let transition = chain
            .advance(Some(&location))
            .expect("a distinct hop within the limit should be accepted");
        ensure!(
            transition.hop == hop + 1,
            "hop {hop} should be numbered {}, got {}",
            hop + 1,
            transition.hop,
        );
    }

    let refused = chain
        .advance(Some("/hop/overflow"))
        .expect_err("the hop after the limit must be refused");
    ensure!(
        matches!(
            refused,
            RedirectRejection::LimitExceeded { limit, .. } if limit == FETCH_REDIRECT_LIMIT
        ),
        "the hop after the limit must report the configured limit: {refused:?}"
    );
    Ok(())
}

/// Verify a repeated target is refused as a loop without another dispatch.
#[rstest]
fn revisited_target_is_refused_as_a_loop(chain_setup: Result<ChainSetup>) -> Result<()> {
    let ChainSetup { base, policy } = chain_setup?;
    let mut chain = RedirectChain::new(&base, &policy);

    chain
        .advance(Some("/once"))
        .expect("the first hop should be accepted");
    let refused = chain
        .advance(Some("/once"))
        .expect_err("revisiting a target must be refused");
    ensure!(
        matches!(refused, RedirectRejection::Loop { .. }),
        "a repeated target must be reported as a loop: {refused:?}"
    );
    ensure!(
        chain.hops() == 1,
        "a refused loop must not advance the chain, hops = {}",
        chain.hops(),
    );
    Ok(())
}

/// Verify missing and unresolvable locations are refused before any request.
#[rstest]
#[case(None, "missing")]
#[case(Some("http://[::1"), "invalid")]
fn unusable_locations_are_refused(
    chain_setup: Result<ChainSetup>,
    #[case] location: Option<&str>,
    #[case] expected: &str,
) -> Result<()> {
    let ChainSetup { base, policy } = chain_setup?;
    let mut chain = RedirectChain::new(&base, &policy);

    let refused = chain
        .advance(location)
        .expect_err("an unusable location must be refused");
    let matched = matches!(
        (&refused, expected),
        (RedirectRejection::LocationMissing { .. }, "missing")
            | (RedirectRejection::LocationInvalid { .. }, "invalid")
    );
    ensure!(matched, "unexpected rejection: {refused:?}");
    ensure!(
        chain.hops() == 0,
        "an unusable location must not advance the chain, hops = {}",
        chain.hops(),
    );
    Ok(())
}

proptest! {
    /// Verify the hop limit and dispatch record hold for generated locations.
    #[test]
    fn generated_locations_respect_the_hop_limit(locations in generated_locations()) {
        let base = initial_url().expect("initial URL should parse");
        let policy = policy_for_hosts(&["allowed.example"]).expect("policy should build");
        let mut chain = RedirectChain::new(&base, &policy);
        let mut dispatched = vec![base];
        let mut accepted = 0_usize;

        for location in &locations {
            let Ok(transition) = chain.advance(Some(location)) else {
                continue;
            };
            accepted += 1;
            prop_assert!(
                accepted <= FETCH_REDIRECT_LIMIT,
                "accepted {accepted} redirects for {locations:?}"
            );
            prop_assert_eq!(transition.hop, accepted);
            prop_assert!(
                !dispatched.contains(&transition.next_url),
                "repeated a dispatched URL for {:?}", location
            );
            dispatched.push(transition.next_url);
        }
        prop_assert_eq!(accepted, chain.hops());
    }

    /// Verify policy decides every hop: accepted targets are permitted and
    /// refused targets are genuinely refused by the same policy.
    #[test]
    fn policy_decides_every_hop(targets in prop::collection::vec(generated_targets(), 1..6)) {
        let base = initial_url().expect("initial URL should parse");
        let policy = policy_for_hosts(&["allowed.example"])
            .expect("policy should build")
            .block_host("blocked.example")
            .expect("blocked.example should be a valid pattern");
        let mut chain = RedirectChain::new(&base, &policy);

        for target in &targets {
            match chain.advance(Some(target)) {
                Ok(transition) => {
                    prop_assert!(
                        policy.evaluate(&transition.next_url).is_ok(),
                        "accepted a target the policy refuses: {}",
                        transition.next_url
                    );
                }
                Err(RedirectRejection::Policy { target: refused, .. }) => {
                    prop_assert!(
                        policy.evaluate(&refused).is_err(),
                        "refused a target the policy permits: {refused}"
                    );
                }
                Err(_) => {}
            }
        }
    }

    /// Verify credentials survive a same-origin hop and never cross one.
    #[test]
    fn cross_origin_hops_drop_credentials(
        host in prop_oneof![Just("allowed.example"), Just("other.example")],
        user in "[a-z]{1,4}",
        secret in "[a-z]{1,4}",
    ) {
        let current = Url::parse(&format!("http://{user}:{secret}@allowed.example/start"))
            .expect("credentialed URL should parse");
        let policy = policy_for_hosts(&["allowed.example", "other.example"])
            .expect("policy should build");
        let mut chain = RedirectChain::new(&current, &policy);
        let location = format!("http://{user}:{secret}@{host}/next");

        let transition = chain
            .advance(Some(&location))
            .expect("both generated hosts are allowlisted");
        if transition.next_url.origin() == current.origin() {
            prop_assert_eq!(transition.next_url.username(), user.as_str());
            prop_assert!(transition.next_url.password().is_some());
        } else {
            prop_assert!(transition.next_url.username().is_empty());
            prop_assert!(transition.next_url.password().is_none());
        }
    }
}
