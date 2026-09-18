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

/// Resolve one raw location against the chain's current URL.
///
/// Reading the `Location` header and resolving it is the adapter's job, so the
/// chain only ever sees a resolved target. These cases build their targets the
/// same way, which keeps the tests honest about the boundary: nothing below
/// asks the chain to parse a header it no longer receives.
///
/// # Errors
///
/// Returns an error when `location` cannot be joined to the current URL.
fn resolve_against(chain: &RedirectChain<'_>, location: &str) -> Result<Url> {
    chain
        .current_url()
        .join(location)
        .with_context(|| format!("test location should resolve: {location}"))
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
        let target = resolve_against(&chain, location)?;
        let Ok(transition) = chain.advance(target) else {
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
        let target = resolve_against(&chain, &format!("/hop/{hop}"))?;
        let transition = chain
            .advance(target)
            .expect("a distinct hop within the limit should be accepted");
        ensure!(
            transition.hop == hop + 1,
            "hop {hop} should be numbered {}, got {}",
            hop + 1,
            transition.hop,
        );
    }

    let overflow = resolve_against(&chain, "/hop/overflow")?;
    let refused = chain
        .advance(overflow)
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

    let first = resolve_against(&chain, "/once")?;
    chain.advance(first).expect("the first hop should be accepted");
    let repeat = resolve_against(&chain, "/once")?;
    let refused = chain
        .advance(repeat)
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

/// Verify a redirect that only changes the fragment is refused as a loop.
///
/// A fragment is never sent to the server, so `/once#a` and `/once#b` name the
/// same request. Keying loop detection on the fragment-bearing URL would accept
/// the second redirect and repeat that request until the hop limit stopped it.
#[rstest]
fn fragment_only_redirect_is_refused_as_a_loop(chain_setup: Result<ChainSetup>) -> Result<()> {
    let ChainSetup { base, policy } = chain_setup?;
    ensure!(
        base.fragment().is_none(),
        "the chain must start from a URL without a fragment: {base}",
    );
    let mut chain = RedirectChain::new(&base, &policy);

    let first = resolve_against(&chain, "/once#a")?;
    let accepted = chain
        .advance(first)
        .expect("the first fragment-bearing hop should be accepted");
    ensure!(
        accepted.next_url.fragment() == Some("a"),
        "an accepted target must keep its fragment: {}",
        accepted.next_url,
    );

    let second = resolve_against(&chain, "/once#b")?;
    let refused = chain
        .advance(second)
        .expect_err("a fragment-only change must be refused as a loop");
    ensure!(
        matches!(refused, RedirectRejection::Loop { .. }),
        "a fragment-only change must be reported as a loop: {refused:?}",
    );
    ensure!(
        matches!(&refused, RedirectRejection::Loop { target } if target.fragment() == Some("b")),
        "the refusal must keep the differing fragment for diagnostics: {refused:?}",
    );
    ensure!(
        chain.hops() == 1,
        "a refused fragment-only redirect must not advance the chain, hops = {}",
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
            let Ok(target) = chain.current_url().join(location) else {
                continue;
            };
            let Ok(transition) = chain.advance(target) else {
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

        for raw in &targets {
            let Ok(target) = Url::parse(raw) else {
                continue;
            };
            match chain.advance(target) {
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

        let target = Url::parse(&location).expect("generated credentialed URL should parse");
        let transition = chain
            .advance(target)
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
