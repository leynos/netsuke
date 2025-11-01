//! Configuration-related Cucumber steps for stdlib scenarios.

use crate::CliWorld;
use anyhow::Result;
use cucumber::given;

#[given(regex = r#"^the stdlib fetch response limit is (\d+) bytes$"#)]
pub(crate) fn configure_fetch_limit(world: &mut CliWorld, limit: u64) -> Result<()> {
    world.stdlib_fetch_max_bytes = Some(limit);
    Ok(())
}
