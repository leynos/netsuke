//! Cucumber test runner and world state.

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use cucumber::World;
use netsuke::stdlib::{NetworkPolicy, StdlibState};
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
use std::{collections::HashMap, ffi::OsString, net::TcpListener};
use test_support::{PathGuard, env::restore_many, http};

/// Shared state for Cucumber scenarios.
#[derive(Debug, Default, World)]
pub struct CliWorld {
    /// Parsed CLI configuration passed into the runner.
    pub cli: Option<netsuke::cli::Cli>,
    /// Error message captured when CLI parsing fails.
    pub cli_error: Option<String>,
    /// Fully parsed manifest for the current scenario.
    pub manifest: Option<netsuke::ast::NetsukeManifest>,
    /// Error text captured when manifest loading fails.
    pub manifest_error: Option<String>,
    /// Build graph derived from the active manifest.
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
    pub stdlib_state: Option<StdlibState>,
    /// Quoted command string for stdlib shell scenarios.
    pub stdlib_command: Option<String>,
    /// Custom network policy applied during stdlib rendering scenarios.
    pub stdlib_policy: Option<NetworkPolicy>,
    /// Maximum fetch response size configured for the active scenario.
    pub stdlib_fetch_max_bytes: Option<u64>,
    /// Last HTTP server fixture started by stdlib steps.
    pub http_server: Option<http::HttpServer>,
    /// URL exposed by the active HTTP server fixture.
    pub stdlib_url: Option<String>,
    /// Snapshot of pre-scenario values for environment variables that were overridden.
    /// Stores the original value (`Some`) or `None` if the variable was previously unset.
    pub env_vars: HashMap<String, Option<OsString>>,
}

mod steps;

/// Controls how the HTTP server fixture teardown behaves when the host cannot
/// be extracted from the captured stdlib URL.
#[derive(Copy, Clone)]
enum HttpShutdownMode {
    /// Panic if the host cannot be extracted, ensuring test failures surface.
    Strict,
    /// Log a warning and skip shutdown when the host is unavailable.
    /// Prevents the test process from hanging when no host is present.
    Lenient,
}

impl CliWorld {
    /// Start a new HTTP server fixture, replacing any existing instance.
    ///
    /// The previous server is shut down in strict mode so failures surface
    /// immediately. The newly spawned server URL and handle are stored for
    /// later assertions and teardown.
    pub(crate) fn start_http_server(&mut self, body: String) -> Result<()> {
        self.shutdown_http_server_with(HttpShutdownMode::Strict);
        let (url, server) =
            http::spawn_http_server(body).context("spawn HTTP server for stdlib steps")?;
        self.stdlib_url = Some(url);
        self.http_server = Some(server);
        Ok(())
    }

    /// Shut down the active HTTP server fixture, tolerating missing host data.
    ///
    /// Lenient mode avoids panicking when the captured URL cannot be parsed.
    /// This allows teardown to continue for scenarios that omit the host
    /// component intentionally.
    pub(crate) fn shutdown_http_server(&mut self) {
        self.shutdown_http_server_with(HttpShutdownMode::Lenient);
    }

    /// Returns the host component of the active stdlib HTTP fixture URL.
    ///
    /// The caller verifies that the URL exposes a host suitable for
    /// cooperative shutdown; [`HttpServer::join`](test_support::http::HttpServer::join)
    /// performs the actual unblocking internally.
    fn extract_host_from_stdlib_url(&self) -> Option<&str> {
        self.stdlib_url
            .as_deref()
            .and_then(steps::stdlib_steps::server_host)
    }

    /// Restore any environment variables overridden during the scenario.
    ///
    /// Values return to their prior state (or are unset if absent); the guard
    /// map is cleared afterwards.
    fn restore_environment(&mut self) {
        if self.env_vars.is_empty() {
            return;
        }

        restore_many(self.env_vars.drain().collect());
    }

    /// Shut down the active HTTP server fixture according to the supplied mode.
    ///
    /// When the stdlib URL contains a host, the server is always joined.
    /// If the host cannot be extracted, strict mode panics while lenient mode
    /// warns and leaves the server to drop naturally.
    #[expect(
        clippy::panic,
        reason = "strict teardown must visibly fail when host extraction breaks"
    )]
    fn shutdown_http_server_with(&mut self, mode: HttpShutdownMode) {
        let Some(server) = self.http_server.take() else {
            self.stdlib_url = None;
            return;
        };

        if self.extract_host_from_stdlib_url().is_some() {
            if let Err(err) = server.join() {
                tracing::warn!("HTTP server thread panicked: {err:?}");
            }
            self.stdlib_url = None;
            return;
        }

        match mode {
            HttpShutdownMode::Strict => panic!(
                "Cannot extract host from stdlib_url; server teardown will hang. URL: {:?}",
                self.stdlib_url
            ),
            HttpShutdownMode::Lenient => {
                tracing::warn!(
                    "Warning: Cannot extract host from stdlib_url; skipping server shutdown to avoid hang. URL: {:?}",
                    self.stdlib_url
                );
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
    if let Err(err) = TcpListener::bind(("127.0.0.1", 0)) {
        tracing::warn!(
            "Skipping Cucumber tests: cannot bind TCP listener on this platform ({err})"
        );
        return;
    }

    CliWorld::run("tests/features").await;
    #[cfg(unix)]
    {
        if block_device_exists() {
            CliWorld::run("tests/features_unix").await;
        } else {
            tracing::warn!("No block device in /dev; skipping Unix file-system features.");
        }
    }
}
