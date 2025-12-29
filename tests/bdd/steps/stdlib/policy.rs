//! Steps for customising the stdlib network policy during behavioural tests.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::types::{HostName, Scheme};
use anyhow::Result;
use netsuke::stdlib::NetworkPolicy;
use rstest_bdd_macros::given;

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Apply a modification function to the current policy and store the result.
///
/// Takes the current policy from the world (or default if none), applies the
/// provided modification closure, and stores the result back. The closure may
/// return any error type that converts to `anyhow::Error`.
fn modify_policy<F, E>(world: &TestWorld, f: F) -> Result<()>
where
    F: FnOnce(NetworkPolicy) -> Result<NetworkPolicy, E>,
    E: Into<anyhow::Error>,
{
    let policy = world.stdlib_policy.take_value().unwrap_or_default();
    let modified = f(policy).map_err(Into::into)?;
    world.stdlib_policy.set_value(modified);
    Ok(())
}

/// Apply an infallible modification function to the current policy and store
/// the result.
fn modify_policy_infallible<F>(world: &TestWorld, f: F)
where
    F: FnOnce(NetworkPolicy) -> NetworkPolicy,
{
    let policy = world.stdlib_policy.take_value().unwrap_or_default();
    let modified = f(policy);
    world.stdlib_policy.set_value(modified);
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("the stdlib network policy allows scheme {scheme:string}")]
pub(crate) fn allow_scheme(world: &TestWorld, scheme: &str) -> Result<()> {
    let scheme_type = Scheme::new(scheme);
    modify_policy(world, |p| p.allow_scheme(scheme_type.as_str()))
}

#[given("the stdlib network policy allows host {host:string}")]
pub(crate) fn allow_host(world: &TestWorld, host: &str) -> Result<()> {
    let host_name = HostName::new(host);
    modify_policy(world, |p| p.allow_hosts([host_name.as_str().to_owned()]))
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[given("the stdlib network policy blocks all hosts by default")]
pub(crate) fn default_deny(world: &TestWorld) -> Result<()> {
    modify_policy_infallible(world, NetworkPolicy::deny_all_hosts);
    Ok(())
}

#[given("the stdlib network policy blocks host {host:string}")]
pub(crate) fn block_host(world: &TestWorld, host: &str) -> Result<()> {
    let host_name = HostName::new(host);
    modify_policy(world, |p| p.block_host(host_name.as_str()))
}
