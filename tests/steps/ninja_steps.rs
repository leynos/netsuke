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

#[allow(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the ninja file contains {string}")]
fn ninja_contains(world: &mut CliWorld, text: String) {
    let ninja = world
        .ninja
        .as_ref()
        .expect("ninja content should be available");
    assert!(ninja.contains(&text));
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "shlex splitting command {int} yields {string}")]
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

#[allow(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "shlex splitting the command yields {string}")]
fn ninja_first_command_tokens(world: &mut CliWorld, expected: String) {
    ninja_command_tokens(world, 2, expected);
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "ninja generation fails with {string}")]
fn ninja_generation_fails(world: &mut CliWorld, text: String) {
    let err = world
        .ninja_error
        .as_ref()
        .expect("ninja error should be available");
    assert!(err.contains(&text));
}
