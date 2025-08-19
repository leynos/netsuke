//! Cucumber test runner and world state.

use cucumber::World;
use std::{collections::HashMap, ffi::OsString};
use test_support::{
    PathGuard,
    env::{remove_var, set_var},
};

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
    /// Guard that restores `PATH` after each scenario.
    pub path_guard: Option<PathGuard>,
    /// Snapshot of pre-scenario values for environment variables that were overridden.
    /// Stores the original value (`Some`) or `None` if the variable was previously unset.
    pub env_vars: HashMap<String, Option<OsString>>,
}

mod steps;

impl Drop for CliWorld {
    fn drop(&mut self) {
        if self.env_vars.is_empty() {
            return;
        }
        for (key, val) in self.env_vars.drain() {
            if let Some(v) = val {
                let _ = set_var(&key, &v);
            } else {
                let _ = remove_var(&key);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    CliWorld::run("tests/features").await;
}
