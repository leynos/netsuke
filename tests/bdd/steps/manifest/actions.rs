//! Action-specific step definitions for manifest parsing scenarios.

use super::assert_action_console;
use crate::bdd::fixtures::TestWorld;
use anyhow::Result;
use rstest_bdd_macros::then;

/// Assert that an action requests Ninja's console pool.
///
/// # Errors
///
/// Returns an error when the action is missing or does not request the pool.
#[then("the action {index:usize} requests console scheduling")]
fn action_requests_console_scheduling(world: &TestWorld, index: usize) -> Result<()> {
    assert_action_console(world, index, true)
}

/// Assert that an action keeps ordinary Ninja scheduling.
///
/// # Errors
///
/// Returns an error when the action is missing or requests the pool.
#[then("the action {index:usize} does not request console scheduling")]
fn action_does_not_request_console_scheduling(world: &TestWorld, index: usize) -> Result<()> {
    assert_action_console(world, index, false)
}
