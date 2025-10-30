//! Steps for customising the stdlib network policy during behavioural tests.

use crate::CliWorld;
use anyhow::Result;
use cucumber::given;

fn take_policy(world: &mut CliWorld) -> netsuke::stdlib::NetworkPolicy {
    world.stdlib_policy.clone().unwrap_or_default()
}

fn store_policy(world: &mut CliWorld, policy: netsuke::stdlib::NetworkPolicy) {
    world.stdlib_policy = Some(policy);
}

#[given(regex = r#"^the stdlib network policy allows scheme "(.+)"$"#)]
pub(crate) fn allow_scheme(world: &mut CliWorld, scheme: String) -> Result<()> {
    let policy = take_policy(world).allow_scheme(scheme)?;
    store_policy(world, policy);
    Ok(())
}

#[given(regex = r#"^the stdlib network policy allows host "(.+)"$"#)]
pub(crate) fn allow_host(world: &mut CliWorld, host: String) -> Result<()> {
    let policy = take_policy(world).allow_hosts([host])?;
    store_policy(world, policy);
    Ok(())
}

#[given("the stdlib network policy blocks all hosts by default")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "Step handlers use Result for ? ergonomics and uniform signatures"
)]
pub(crate) fn default_deny(world: &mut CliWorld) -> Result<()> {
    let policy = take_policy(world).deny_all_hosts();
    store_policy(world, policy);
    Ok(())
}

#[given(regex = r#"^the stdlib network policy blocks host "(.+)"$"#)]
pub(crate) fn block_host(world: &mut CliWorld, host: String) -> Result<()> {
    let policy = take_policy(world).block_host(host)?;
    store_policy(world, policy);
    Ok(())
}
