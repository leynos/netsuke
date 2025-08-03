use cucumber::World;

#[derive(Debug, Default, World)]
pub struct CliWorld {
    pub cli: Option<netsuke::cli::Cli>,
    pub cli_error: Option<String>,
    pub manifest: Option<netsuke::ast::NetsukeManifest>,
    pub manifest_error: Option<String>,
    pub build_graph: Option<netsuke::ir::BuildGraph>,
    pub ninja: Option<String>,
    pub run_status: Option<bool>,
    pub run_error: Option<String>,
    pub temp: Option<tempfile::TempDir>,
}

mod steps;

#[tokio::main]
async fn main() {
    CliWorld::run("tests/features").await;
}
