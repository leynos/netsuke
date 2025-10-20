//! Cucumber test runner and world state.

use camino::Utf8PathBuf;
use cucumber::World;
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
use std::{collections::HashMap, ffi::OsString, net::TcpStream, thread::JoinHandle};
use test_support::{PathGuard, env::restore_many, http};

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
    /// Root directory for stdlib scenarios.
    pub stdlib_root: Option<Utf8PathBuf>,
    /// Captured output from the last stdlib render.
    pub stdlib_output: Option<String>,
    /// Error from the last stdlib render.
    pub stdlib_error: Option<String>,
    /// Stdlib impurity state captured for the last render.
    pub stdlib_state: Option<netsuke::stdlib::StdlibState>,
    /// Quoted command string for stdlib shell scenarios.
    pub stdlib_command: Option<String>,
    /// Last HTTP server fixture started by stdlib steps.
    pub http_server: Option<JoinHandle<()>>,
    /// URL exposed by the active HTTP server fixture.
    pub stdlib_url: Option<String>,
    /// Snapshot of pre-scenario values for environment variables that were overridden.
    /// Stores the original value (`Some`) or `None` if the variable was previously unset.
    pub env_vars: HashMap<String, Option<OsString>>,
}

mod steps;

#[derive(Copy, Clone)]
enum HttpShutdownMode {
    Strict,
    Lenient,
}

impl CliWorld {
    pub(crate) fn start_http_server(&mut self, body: String) {
        self.shutdown_http_server_with(HttpShutdownMode::Strict);
        let (url, handle) = http::spawn_http_server(body);
        self.stdlib_url = Some(url);
        self.http_server = Some(handle);
    }

    pub(crate) fn shutdown_http_server(&mut self) {
        self.shutdown_http_server_with(HttpShutdownMode::Lenient);
    }

    /// Returns the host component of the active stdlib HTTP fixture URL.
    ///
    /// The caller uses the host to unblock the listener during teardown.
    fn extract_host_from_stdlib_url(&self) -> Option<&str> {
        self.stdlib_url
            .as_deref()
            .and_then(steps::stdlib_steps::server_host)
    }

    fn restore_environment(&mut self) {
        if self.env_vars.is_empty() {
            return;
        }

        restore_many(self.env_vars.drain().collect());
    }

    fn shutdown_http_server_with(&mut self, mode: HttpShutdownMode) {
        let Some(handle) = self.http_server.take() else {
            self.stdlib_url = None;
            return;
        };

        if let Some(host) = self.extract_host_from_stdlib_url() {
            let _ = TcpStream::connect(host);
            let _ = handle.join();
            self.stdlib_url = None;
            return;
        }

        match mode {
            HttpShutdownMode::Strict => panic!(
                "Cannot extract host from stdlib_url; server teardown will hang. URL: {:?}",
                self.stdlib_url
            ),
            HttpShutdownMode::Lenient => {
                eprintln!(
                    "Warning: Cannot extract host from stdlib_url; skipping server shutdown to avoid hang. URL: {:?}",
                    self.stdlib_url
                );
                drop(handle);
            }
        }
        self.stdlib_url = None;
    }
}

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
        self.shutdown_http_server();
        self.restore_environment();
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
