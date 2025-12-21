//! Steps for customising the stdlib network policy during behavioural tests.

use crate::bdd::fixtures::{RefCellOptionExt, strip_quotes, with_world};
use anyhow::Result;
use netsuke::stdlib::NetworkPolicy;
use rstest_bdd_macros::given;

fn take_policy() -> NetworkPolicy {
    with_world(|world| world.stdlib_policy.take_value().unwrap_or_default())
}

fn store_policy(policy: NetworkPolicy) {
    with_world(|world| {
        world.stdlib_policy.set_value(policy);
    });
}

#[given("the stdlib network policy allows scheme {scheme}")]
pub(crate) fn allow_scheme(scheme: String) -> Result<()> {
    let scheme = strip_quotes(&scheme).to_string();
    let policy = take_policy().allow_scheme(scheme)?;
    store_policy(policy);
    Ok(())
}

#[given("the stdlib network policy allows host {host}")]
pub(crate) fn allow_host(host: String) -> Result<()> {
    let host = strip_quotes(&host).to_string();
    let policy = take_policy().allow_hosts([host])?;
    store_policy(policy);
    Ok(())
}

#[given("the stdlib network policy blocks all hosts by default")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "Step handlers use Result for ? ergonomics and uniform signatures"
)]
pub(crate) fn default_deny() -> Result<()> {
    let policy = take_policy().deny_all_hosts();
    store_policy(policy);
    Ok(())
}

#[given("the stdlib network policy blocks host {host}")]
pub(crate) fn block_host(host: String) -> Result<()> {
    let host = strip_quotes(&host).to_string();
    let policy = take_policy().block_host(host)?;
    store_policy(policy);
    Ok(())
}
