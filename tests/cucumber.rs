//! Cucumber test runner.

mod world;
use cucumber::World as _;
pub use world::CliWorld;

mod steps;
mod support;

#[tokio::main]
async fn main() {
    CliWorld::run("tests/features").await;
}
