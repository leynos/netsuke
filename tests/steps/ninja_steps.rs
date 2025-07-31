//! Step definitions for Ninja file generation scenarios.

use crate::CliWorld;
use cucumber::{then, when};
use netsuke::ninja_gen;

#[when("the ninja file is generated")]
fn generate_ninja(world: &mut CliWorld) {
    let graph = world.build_graph.as_ref().expect("graph");
    world.ninja = Some(ninja_gen::generate(graph));
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the ninja file contains {string}")]
fn ninja_contains(world: &mut CliWorld, text: String) {
    let ninja = world.ninja.as_ref().expect("ninja");
    assert!(ninja.contains(&text));
}
