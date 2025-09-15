//! Step definitions for Ninja file generation scenarios.

use crate::CliWorld;
use cucumber::{then, when};
use netsuke::ninja_gen;

#[when("the ninja file is generated")]
fn generate_ninja(world: &mut CliWorld) {
    let graph = world
        .build_graph
        .as_ref()
        .expect("build graph should be available");
    match ninja_gen::generate(graph) {
        Ok(n) => {
            world.ninja = Some(n);
            world.ninja_error = None;
        }
        Err(e) => {
            world.ninja = None;
            world.ninja_error = Some(e.to_string());
        }
    }
}

#[then(expr = "the ninja file contains {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
fn ninja_contains(world: &mut CliWorld, text: String) {
    let ninja = world
        .ninja
        .as_ref()
        .expect("ninja content should be available");
    assert!(ninja.contains(&text));
}

#[then(expr = "shlex splitting command {int} yields {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
fn ninja_command_tokens(world: &mut CliWorld, index: usize, expected: String) {
    let ninja = world
        .ninja
        .as_ref()
        .expect("ninja content should be available");
    let commands: Vec<&str> = ninja
        .lines()
        .filter(|l| l.trim_start().starts_with("command ="))
        .collect();
    let line = commands.get(index - 1).expect("command index within range");
    let command = line.trim_start().trim_start_matches("command = ");
    let words = shlex::split(command).expect("split command");
    let expected: Vec<String> = expected
        .split(',')
        .map(|w| w.trim().replace("\\n", "\n"))
        .collect();
    assert_eq!(words, expected);
}

#[then(expr = "shlex splitting the command yields {string}")]
fn ninja_first_command_tokens(world: &mut CliWorld, expected: String) {
    ninja_command_tokens(world, 2, expected);
}

#[then(expr = "ninja generation fails with {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
fn ninja_generation_fails(world: &mut CliWorld, text: String) {
    let err = world
        .ninja_error
        .as_ref()
        .expect("ninja error should be available");
    assert!(err.contains(&text));
}

#[then("ninja generation fails mentioning the removed action id")]
fn ninja_generation_fails_with_removed_action_id(world: &mut CliWorld) {
    let err = world
        .ninja_error
        .as_ref()
        .expect("ninja error should be available");
    let id = world
        .removed_action_id
        .as_ref()
        .expect("removed action id should be available");
    assert!(err.contains(id));
}
