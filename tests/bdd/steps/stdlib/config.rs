//! Configuration-related BDD steps for stdlib scenarios.

use crate::bdd::fixtures::TestWorld;
use rstest_bdd_macros::given;

#[given("the stdlib fetch response limit is {limit:u64} bytes")]
pub(crate) fn configure_fetch_limit(world: &TestWorld, limit: u64) {
    world.stdlib_fetch_max_bytes.set(limit);
}

#[given("the stdlib command output limit is {limit:u64} bytes")]
pub(crate) fn configure_command_output_limit(world: &TestWorld, limit: u64) {
    world.stdlib_command_max_output_bytes.set(limit);
}

#[given("the stdlib command stream limit is {limit:u64} bytes")]
pub(crate) fn configure_command_stream_limit(world: &TestWorld, limit: u64) {
    world.stdlib_command_stream_max_bytes.set(limit);
}

#[given("the stdlib template text contains {lines:usize} lines of {line:string}")]
pub(crate) fn configure_stdlib_text(world: &TestWorld, lines: usize, line: &str) {
    let mut text = String::with_capacity(line.len().saturating_add(1).saturating_mul(lines));
    for _ in 0..lines {
        text.push_str(line);
        text.push('\n');
    }
    world.stdlib_text.set(text);
}
