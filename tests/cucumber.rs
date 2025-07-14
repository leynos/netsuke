use assert_cmd::Command;
use cucumber::{World, given, then, when};
use std::process::Output;

#[derive(Debug, Default, World)]
pub struct CliWorld {
    output: Option<Output>,
}

#[given("netsuke is built")]
fn netsuke_built(_world: &mut CliWorld) {}

#[when(expr = "I run netsuke {word}")]
fn i_run_netsuke(world: &mut CliWorld, flag: String) {
    let output = Command::cargo_bin("netsuke")
        .expect("binary")
        .arg(flag)
        .output()
        .expect("runs");
    world.output = Some(output);
}

#[then("the process exits successfully")]
fn exits_success(world: &mut CliWorld) {
    let status = world.output.as_ref().expect("output").status;
    assert!(status.success());
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "cucumber requires owned String"
)]
#[then(expr = "stdout contains {string}")]
fn stdout_contains(world: &mut CliWorld, text: String) {
    let out = world.output.as_ref().expect("output");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(&text));
}

#[tokio::main]
async fn main() {
    CliWorld::run("tests/features").await;
}
