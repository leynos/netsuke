//! Steps for customising the stdlib network policy during behavioural tests.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use anyhow::Result;
use netsuke::stdlib::NetworkPolicy;
use rstest_bdd_macros::given;

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn take_policy(world: &TestWorld) -> NetworkPolicy {
    world.stdlib_policy.take_value().unwrap_or_default()
}

fn store_policy(world: &TestWorld, policy: NetworkPolicy) {
    world.stdlib_policy.set_value(policy);
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("the stdlib network policy allows scheme {scheme:string}")]
pub(crate) fn allow_scheme(world: &TestWorld, scheme: &str) -> Result<()> {
    let policy = take_policy(world).allow_scheme(scheme)?;
    store_policy(world, policy);
    Ok(())
}

#[given("the stdlib network policy allows host {host:string}")]
pub(crate) fn allow_host(world: &TestWorld, host: &str) -> Result<()> {
    let policy = take_policy(world).allow_hosts([host.to_string()])?;
    store_policy(world, policy);
    Ok(())
}

#[given("the stdlib network policy blocks all hosts by default")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "Step handlers use Result for ? ergonomics and uniform signatures"
)]
pub(crate) fn default_deny(world: &TestWorld) -> Result<()> {
    let policy = take_policy(world).deny_all_hosts();
    store_policy(world, policy);
    Ok(())
}

#[given("the stdlib network policy blocks host {host:string}")]
pub(crate) fn block_host(world: &TestWorld, host: &str) -> Result<()> {
    let policy = take_policy(world).block_host(host)?;
    store_policy(world, policy);
    Ok(())
}
