//! Configuration-related Cucumber steps for stdlib scenarios.

use super::types::ExpectedOutput;
use crate::CliWorld;
use cucumber::given;

#[given(regex = r"^the stdlib fetch response limit is (\d+) bytes$")]
pub(crate) const fn configure_fetch_limit(world: &mut CliWorld, limit: u64) {
    world.stdlib_fetch_max_bytes = Some(limit);
}

#[given(regex = r"^the stdlib command output limit is (\d+) bytes$")]
pub(crate) const fn configure_command_output_limit(world: &mut CliWorld, limit: u64) {
    world.stdlib_command_max_output_bytes = Some(limit);
}

#[given(regex = r"^the stdlib command stream limit is (\d+) bytes$")]
pub(crate) const fn configure_command_stream_limit(world: &mut CliWorld, limit: u64) {
    world.stdlib_command_stream_max_bytes = Some(limit);
}

#[given(regex = r#"^the stdlib template text contains (\d+) lines of "(.+)"$"#)]
pub(crate) fn configure_stdlib_text(world: &mut CliWorld, lines: usize, line: ExpectedOutput) {
    let line = line.into_inner();
    let mut text = String::with_capacity(line.len().saturating_mul(lines + 1));
    for _ in 0..lines {
        text.push_str(&line);
        text.push('\n');
    }
    world.stdlib_text = Some(text);
}
