//! Configuration-related BDD steps for stdlib scenarios.

use crate::bdd::fixtures::{strip_quotes, with_world};
use rstest_bdd_macros::given;

#[given("the stdlib fetch response limit is {limit:u64} bytes")]
pub(crate) fn configure_fetch_limit(limit: u64) {
    with_world(|world| {
        world.stdlib_fetch_max_bytes.set(limit);
    });
}

#[given("the stdlib command output limit is {limit:u64} bytes")]
pub(crate) fn configure_command_output_limit(limit: u64) {
    with_world(|world| {
        world.stdlib_command_max_output_bytes.set(limit);
    });
}

#[given("the stdlib command stream limit is {limit:u64} bytes")]
pub(crate) fn configure_command_stream_limit(limit: u64) {
    with_world(|world| {
        world.stdlib_command_stream_max_bytes.set(limit);
    });
}

#[given("the stdlib template text contains {lines:usize} lines of {line}")]
pub(crate) fn configure_stdlib_text(lines: usize, line: String) {
    let line = strip_quotes(&line);
    let mut text = String::with_capacity(line.len().saturating_mul(lines + 1));
    for _ in 0..lines {
        text.push_str(line);
        text.push('\n');
    }
    with_world(|world| {
        world.stdlib_text.set(text);
    });
}
