//! Cucumber test runner and world state.

use cucumber::World;
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
use std::{collections::HashMap, ffi::OsString};
use test_support::{PathGuard, env::restore_many};

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
    /// Error message from Ninja generation.
    pub ninja_error: Option<String>,
    /// Identifier of the action removed for negative tests.
    pub removed_action_id: Option<String>,
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

#[cfg(unix)]
fn block_device_exists() -> bool {
    std::fs::read_dir("/dev")
        .map(|entries| {
            entries.flatten().any(|e| {
                e.file_type()
                    .map(|ft| ft.is_block_device())
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

impl Drop for CliWorld {
    fn drop(&mut self) {
        if !self.env_vars.is_empty() {
            restore_many(self.env_vars.drain().collect());
        }
    }
}

#[tokio::main]
async fn main() {
    CliWorld::run("tests/features").await;
    #[cfg(unix)]
    {
        if block_device_exists() {
            CliWorld::run("tests/features_unix").await;
        } else {
            eprintln!("No block device in /dev; skipping Unix file-system features.");
        }
    }
}
