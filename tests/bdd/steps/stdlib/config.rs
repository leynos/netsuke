//! Configuration-related BDD steps for stdlib scenarios.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use netsuke::{cli_localization, localization};
use rstest_bdd_macros::given;
use std::sync::Arc;
use test_support::localizer_test_lock;

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

#[given("the localisation locale is {locale:string}")]
pub(crate) fn configure_localisation(
    world: &TestWorld,
    locale: &str,
) -> Result<(), std::sync::PoisonError<std::sync::MutexGuard<'static, ()>>> {
    // Release existing localizer guard first.
    world.localization_guard.take_value();

    // Reuse existing lock if present to avoid deadlock; otherwise acquire a new one.
    // Using take_value().map_or_else() combines the check and take into a single operation.
    let lock = world
        .localization_lock
        .take_value()
        .map_or_else(|| localizer_test_lock(), Ok)?;

    let localizer = cli_localization::build_localizer(Some(locale));
    let guard = localization::set_localizer_for_tests(Arc::from(localizer));
    world.localization_lock.set_value(lock);
    world.localization_guard.set_value(guard);
    Ok(())
}
