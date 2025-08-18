//! Cucumber test runner and world state.

use cucumber::World;
use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
};
use test_support::{PathGuard, env_lock::EnvLock};

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
    /// Environment variables overridden during a scenario.
    pub env_vars: HashMap<String, Option<OsString>>,
}

fn set_var(key: &str, value: &OsStr) {
    // SAFETY: `EnvLock` serialises mutations.
    unsafe { std::env::set_var(key, value) };
}

fn remove_var(key: &str) {
    // SAFETY: `EnvLock` serialises mutations.
    unsafe { std::env::remove_var(key) };
}

mod steps;

impl Drop for CliWorld {
    fn drop(&mut self) {
        if self.env_vars.is_empty() {
            return;
        }
        let _lock = EnvLock::acquire();
        for (key, val) in self.env_vars.drain() {
            if let Some(v) = val {
                set_var(&key, &v);
            } else {
                remove_var(&key);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    CliWorld::run("tests/features").await;
}
