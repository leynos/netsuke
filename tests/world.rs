//! Shared test world for Cucumber scenarios.

use cucumber::World;
use mockable::{Env, MockEnv};

/// Shared state for Cucumber scenarios.
#[derive(World)]
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
    /// Mockable environment access.
    pub env: Box<dyn Env>,
    /// Original `PATH` value restored after each scenario.
    pub original_path: Option<std::ffi::OsString>,
}

impl std::fmt::Debug for CliWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CliWorld")
            .field("cli", &self.cli)
            .field("cli_error", &self.cli_error)
            .field("manifest", &self.manifest)
            .field("manifest_error", &self.manifest_error)
            .field("build_graph", &self.build_graph)
            .field("ninja", &self.ninja)
            .field("run_status", &self.run_status)
            .field("run_error", &self.run_error)
            .field("temp", &self.temp)
            .field("env", &"<env>")
            .field("original_path", &self.original_path)
            .finish()
    }
}

impl Default for CliWorld {
    fn default() -> Self {
        let mut env = MockEnv::new();
        env.expect_raw().returning(|key| std::env::var(key));
        Self {
            cli: None,
            cli_error: None,
            manifest: None,
            manifest_error: None,
            build_graph: None,
            ninja: None,
            run_status: None,
            run_error: None,
            temp: None,
            env: Box::new(env),
            original_path: None,
        }
    }
}

impl Drop for CliWorld {
    fn drop(&mut self) {
        if let Some(path) = self.original_path.take() {
            // SAFETY: Rust 2024 marks `set_var` as unsafe. Dropping `CliWorld`
            // reinstates the original `PATH`, ensuring scenarios cannot leak
            // environment changes into subsequent tests.
            unsafe { std::env::set_var("PATH", path) }
        }
    }
}
