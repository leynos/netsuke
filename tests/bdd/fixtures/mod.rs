//! Fixture modules for BDD scenarios.
//!
//! The `TestWorld` struct holds all state for BDD scenarios. Non-Clone types
//! use `RefCell<Option<T>>` directly, while Clone types use `Slot<T>`.
//!
//! With rstest-bdd 0.3.1+, fixtures are injected directly into step functions
//! as parameters, eliminating the need for thread-local storage.

// The `#[fixture]` macro generates types that cannot have doc comments attached
#![expect(
    missing_docs,
    reason = "Generated fixture types cannot have doc comments attached"
)]

use camino::Utf8PathBuf;
use netsuke::cli::Cli;
use netsuke::localization::LocalizerGuard;
use netsuke::stdlib::{NetworkPolicy, StdlibState as NetsukeStdlibState};
use rstest::fixture;
use rstest_bdd::Slot;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::MutexGuard;
use test_support::PathGuard;
use test_support::env::{NinjaEnvGuard, restore_many};
use test_support::http::HttpServer;

/// Combined test world for all BDD scenarios.
///
/// Non-Clone types are stored in `RefCell<Option<T>>` to allow interior
/// mutability without requiring Clone. Clone-able types use `Slot<T>`.
#[derive(Default)]
pub struct TestWorld {
    // CLI state (non-Clone)
    /// Parsed CLI configuration passed into the runner.
    pub cli: RefCell<Option<Cli>>,
    /// Error message captured when CLI parsing fails.
    pub cli_error: Slot<String>,

    // Manifest state (non-Clone)
    /// Fully parsed manifest for the current scenario.
    pub manifest: RefCell<Option<netsuke::ast::NetsukeManifest>>,
    /// Error text captured when manifest loading fails.
    pub manifest_error: Slot<String>,

    // IR state (non-Clone)
    /// Build graph derived from the active manifest.
    pub build_graph: RefCell<Option<netsuke::ir::BuildGraph>>,
    /// Identifier of the action removed for negative tests.
    pub removed_action_id: Slot<String>,
    /// Error text captured when IR generation fails.
    pub generation_error: Slot<String>,

    // Ninja state (Clone)
    /// Generated Ninja file content.
    pub ninja_content: Slot<String>,
    /// Error message from Ninja generation.
    pub ninja_error: Slot<String>,

    // Process state (mixed)
    /// Status of the last process execution (true for success, false for failure).
    pub run_status: Slot<bool>,
    /// Error message from the last failed process execution.
    pub run_error: Slot<String>,
    /// Captured stdout from the last `netsuke` CLI process invocation.
    pub command_stdout: Slot<String>,
    /// Captured stderr from the last `netsuke` CLI process invocation.
    pub command_stderr: Slot<String>,
    /// Temporary directory handle for test isolation (non-Clone).
    pub temp_dir: RefCell<Option<tempfile::TempDir>>,
    /// Explicit workspace path created by `empty_workspace_at_path` for `-C` flag tests.
    pub workspace_path: RefCell<Option<PathBuf>>,
    /// Guard that restores `PATH` after each scenario (non-Clone).
    pub path_guard: RefCell<Option<PathGuard>>,
    /// Guard that overrides `NINJA_ENV` for deterministic Ninja resolution (non-Clone).
    pub ninja_env_guard: RefCell<Option<NinjaEnvGuard>>,

    // Stdlib state (Clone)
    /// Root directory for stdlib scenarios.
    pub stdlib_root: Slot<Utf8PathBuf>,
    /// Captured output from the last stdlib render.
    pub stdlib_output: Slot<String>,
    /// Error from the last stdlib render.
    pub stdlib_error: Slot<String>,
    /// Stdlib impurity state captured for the last render (non-Clone).
    pub stdlib_state: RefCell<Option<NetsukeStdlibState>>,
    /// Quoted command string for stdlib shell scenarios.
    pub stdlib_command: Slot<String>,
    /// Custom network policy applied during stdlib rendering scenarios (non-Clone).
    pub stdlib_policy: RefCell<Option<NetworkPolicy>>,
    /// Override for the PATH environment variable used by the `which` resolver.
    pub stdlib_path_override: RefCell<Option<OsString>>,
    /// Maximum fetch response size configured for the active scenario.
    pub stdlib_fetch_max_bytes: Slot<u64>,
    /// Maximum captured command output size configured for the scenario.
    pub stdlib_command_max_output_bytes: Slot<u64>,
    /// Maximum streamed command output size configured for the scenario.
    pub stdlib_command_stream_max_bytes: Slot<u64>,
    /// Text payload injected into stdlib templates for streaming scenarios.
    pub stdlib_text: Slot<String>,

    // Localization state (non-Clone)
    /// Lock guarding process-wide localizer mutations during scenarios.
    pub localization_lock: RefCell<Option<MutexGuard<'static, ()>>>,
    /// Localizer guard for scenario-level localization overrides.
    pub localization_guard: RefCell<Option<LocalizerGuard>>,

    // Locale resolution state (Clone)
    /// Locale override supplied via configuration layers for resolution scenarios.
    pub locale_config: Slot<String>,
    /// Locale override supplied via environment layers for resolution scenarios.
    pub locale_env: Slot<String>,
    /// Locale override supplied via CLI layers for resolution scenarios.
    pub locale_cli_override: Slot<String>,
    /// System locale value supplied for resolution scenarios.
    pub locale_system: Slot<String>,
    /// Resolved locale output captured for resolution scenarios.
    pub resolved_locale: Slot<String>,
    /// Localized message output captured for resolution scenarios.
    pub locale_message: Slot<String>,

    // HTTP server state (non-Clone)
    /// Last HTTP server fixture started by stdlib steps.
    pub http_server: RefCell<Option<HttpServer>>,
    /// URL exposed by the active HTTP server fixture.
    pub stdlib_url: Slot<String>,

    // Output mode state (Clone)
    /// Resolved output mode for accessible output scenarios.
    pub output_mode: Slot<String>,
    /// Simulated `NO_COLOR` value for output mode detection scenarios.
    pub simulated_no_color: Slot<String>,
    /// Simulated `TERM` value for output mode detection scenarios.
    pub simulated_term: Slot<String>,

    // Environment state
    /// Snapshot of pre-scenario values for environment variables that were overridden.
    pub env_vars: RefCell<HashMap<String, Option<OsString>>>,
}

impl TestWorld {
    /// Track an environment variable for later restoration.
    pub fn track_env_var(&self, key: String, previous: Option<OsString>) {
        self.env_vars.borrow_mut().entry(key).or_insert(previous);
    }

    /// Restore any environment variables overridden during the scenario.
    fn restore_environment(&self) {
        let vars = std::mem::take(&mut *self.env_vars.borrow_mut());
        if !vars.is_empty() {
            restore_many(vars);
        }
    }

    /// Shut down the active HTTP server fixture.
    pub fn shutdown_http_server(&self) {
        let Some(server) = self.http_server.borrow_mut().take() else {
            self.stdlib_url.clear();
            return;
        };
        let Some(url) = self.stdlib_url.get() else {
            self.stdlib_url.clear();
            return;
        };
        // Validate URL has a host before attempting shutdown
        if server_host(&url).is_some()
            && let Err(err) = server.join()
        {
            tracing::warn!("HTTP server thread panicked: {err:?}");
        }
        self.stdlib_url.clear();
    }
}

impl Drop for TestWorld {
    fn drop(&mut self) {
        self.shutdown_http_server();
        self.ninja_env_guard.borrow_mut().take();
        self.localization_guard.borrow_mut().take();
        self.localization_lock.borrow_mut().take();
        self.restore_environment();
        self.stdlib_text.clear();
    }
}

/// Extract the host component from a URL.
///
/// This function handles only plain HTTP URLs with IPv4 hosts (e.g.,
/// `http://127.0.0.1:8080/path`). It does not support HTTPS, IPv6 addresses
/// in brackets, or other edge cases. This is intentional: the test HTTP
/// server always binds to `127.0.0.1`, so broader URL parsing is unnecessary.
fn server_host(url: &str) -> Option<&str> {
    url.strip_prefix("http://")
        .and_then(|s| s.split('/').next())
        .and_then(|host_port| host_port.split(':').next())
}

/// Fixture providing a fresh `TestWorld` for each scenario.
#[fixture]
pub fn world() -> TestWorld {
    TestWorld::default()
}

/// Helper trait extensions for `RefCell<Option<T>>`.
///
/// Provides ergonomic methods for working with `RefCell<Option<T>>` values
/// in BDD step definitions, enabling interior mutability without requiring `Clone`.
pub trait RefCellOptionExt<T> {
    /// Set the value inside the `RefCell`.
    fn set_value(&self, value: T);
    /// Clear the value inside the `RefCell`, setting it to `None`.
    fn clear_value(&self);
    /// Returns `true` if the `RefCell` contains `Some`.
    fn is_some(&self) -> bool;
    /// Borrow the inner value immutably and apply a function.
    fn with_ref<R>(&self, f: impl FnOnce(&T) -> R) -> Option<R>;
    /// Borrow the inner value mutably and apply a function.
    fn with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R>;
    /// Take the value out of the `RefCell`, leaving `None` in its place.
    fn take_value(&self) -> Option<T>;
}

impl<T> RefCellOptionExt<T> for RefCell<Option<T>> {
    fn set_value(&self, value: T) {
        *self.borrow_mut() = Some(value);
    }

    fn clear_value(&self) {
        *self.borrow_mut() = None;
    }

    fn is_some(&self) -> bool {
        self.borrow().is_some()
    }

    fn with_ref<R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        self.borrow().as_ref().map(f)
    }

    fn with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        self.borrow_mut().as_mut().map(f)
    }

    fn take_value(&self) -> Option<T> {
        self.borrow_mut().take()
    }
}
