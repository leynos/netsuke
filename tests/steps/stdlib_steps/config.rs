//! Configuration-related Cucumber steps for stdlib scenarios.

use crate::CliWorld;
use cucumber::given;

#[given(regex = r"^the stdlib fetch response limit is (\d+) bytes$")]
pub(crate) const fn configure_fetch_limit(world: &mut CliWorld, limit: u64) {
    world.stdlib_fetch_max_bytes = Some(limit);
}
