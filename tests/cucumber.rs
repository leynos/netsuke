use cucumber::World;

#[derive(Debug, Default, World)]
pub struct CliWorld {
    pub cli: Option<netsuke::cli::Cli>,
}

mod steps;

#[tokio::main]
async fn main() {
    CliWorld::run("tests/features").await;
}
