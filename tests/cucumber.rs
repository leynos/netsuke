//! Cucumber test runner and world state.

use cucumber::World;

/// Shared state for Cucumber scenarios.
#[derive(Debug, Default, World)]
pub struct CliWorld {
    pub cli: Option<netsuke::cli::Cli>,
    pub cli_error: Option<String>,
    pub manifest: Option<netsuke::ast::NetsukeManifest>,
    pub manifest_error: Option<String>,
    pub build_graph: Option<netsuke::ir::BuildGraph>,
    /// Generated Ninja file content.
    pub ninja: Option<String>,
    /// Status of the last process execution (true for success, false for
    /// failure).
    pub run_status: Option<bool>,
    /// Error message from the last failed process execution.
    pub run_error: Option<String>,
    /// Temporary directory handle for test isolation.
    pub temp: Option<tempfile::TempDir>,
    /// Original `PATH` value restored after each scenario.
    pub original_path: Option<std::ffi::OsString>,
}

impl Drop for CliWorld {
    fn drop(&mut self) {
        if let Some(path) = self.original_path.take() {
            // SAFETY: nightly marks `set_var` as unsafe; restore path for isolation.
            unsafe {
                std::env::set_var("PATH", path);
            }
        }
    }
}

mod steps;
mod support;

#[tokio::main]
async fn main() {
    CliWorld::run("tests/features").await;
}
