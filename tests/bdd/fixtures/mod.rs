//! Fixture modules for BDD scenarios.
//!
//! The `TestWorld` struct holds all state for BDD scenarios. Non-Clone types
//! use `RefCell<Option<T>>` directly, while Clone types use `Slot<T>`.
//!
//! Since the `scenarios!` macro doesn't support fixture injection, we use
//! thread-local storage to provide the world to steps. Each scenario gets
//! a fresh world initialized at the start.

// The `#[fixture]` macro generates types that cannot have doc comments attached
#![allow(
    missing_docs,
    reason = "Generated fixture types cannot have doc comments attached"
)]

use camino::Utf8PathBuf;
use netsuke::cli::Cli;
use netsuke::stdlib::{NetworkPolicy, StdlibState as NetsukeStdlibState};
use rstest::fixture;
use rstest_bdd::Slot;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsString;
use test_support::PathGuard;
use test_support::env::{NinjaEnvGuard, restore_many};
use test_support::http::HttpServer;

// Thread-local storage for the current scenario's world
thread_local! {
    static WORLD: RefCell<Option<TestWorld>> = const { RefCell::new(None) };
    // Track which scenario owns the current world (by test name)
    static CURRENT_SCENARIO: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Initialize a fresh world for the current scenario.
///
/// This should be called at the start of each scenario to ensure
/// steps have a clean world to work with.
pub fn init_world() {
    WORLD.with(|w| {
        // Drop any existing world to run cleanup
        let _ = w.borrow_mut().take();
        *w.borrow_mut() = Some(TestWorld::default());
    });
}

/// Get the current test/scenario name from the thread.
fn current_scenario_name() -> Option<String> {
    std::thread::current().name().map(String::from)
}

/// Check if we need to reset the world for a new scenario.
///
/// Returns true if the scenario has changed since last access.
fn should_reset_world() -> bool {
    let current = current_scenario_name();
    CURRENT_SCENARIO.with(|stored| {
        let stored_name = stored.borrow();
        match (&*stored_name, &current) {
            (None, _) => true,                                // No scenario recorded yet
            (Some(prev), Some(curr)) if prev != curr => true, // Different scenario
            _ => false,
        }
    })
}

/// Update the stored scenario name.
fn update_scenario_name() {
    let current = current_scenario_name();
    CURRENT_SCENARIO.with(|stored| {
        *stored.borrow_mut() = current;
    });
}

/// Access the current scenario's world, initializing if needed.
///
/// The world is lazily initialized on first access within a test.
/// When a new scenario is detected (via thread name change), the
/// world is automatically reset to ensure test isolation.
///
/// # Panics
///
/// Panics if the world cannot be initialized, which should never happen
/// under normal operation since the world is always created before access.
#[expect(
    clippy::expect_used,
    reason = "World is always initialized before access"
)]
pub fn with_world<R>(f: impl FnOnce(&TestWorld) -> R) -> R {
    let should_reset = should_reset_world();

    // Check if we've entered a new scenario
    if should_reset {
        // Reset the world for the new scenario
        WORLD.with(|w| {
            let _ = w.borrow_mut().take(); // Drop old world, running cleanup
            *w.borrow_mut() = Some(TestWorld::default());
        });
        update_scenario_name();
    }

    WORLD.with(|w| {
        // Lazily initialize the world if not yet created
        if w.borrow().is_none() {
            *w.borrow_mut() = Some(TestWorld::default());
            update_scenario_name();
        }
        let guard = w.borrow();
        let world = guard.as_ref().expect("world should be initialized");
        f(world)
    })
}

/// Clean up the current scenario's world.
///
/// This should be called at the end of each scenario to ensure
/// proper cleanup of resources.
pub fn cleanup_world() {
    WORLD.with(|w| {
        let _ = w.borrow_mut().take();
    });
}

/// Reset the world for a new scenario.
///
/// This is called by the before hook at the start of each scenario
/// to ensure a fresh world state.
pub fn reset_world() {
    WORLD.with(|w| {
        // Drop any existing world to run cleanup (e.g., Drop implementations)
        let _ = w.borrow_mut().take();
        // Create a fresh world
        *w.borrow_mut() = Some(TestWorld::default());
    });
}

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
    /// Maximum fetch response size configured for the active scenario.
    pub stdlib_fetch_max_bytes: Slot<u64>,
    /// Maximum captured command output size configured for the scenario.
    pub stdlib_command_max_output_bytes: Slot<u64>,
    /// Maximum streamed command output size configured for the scenario.
    pub stdlib_command_stream_max_bytes: Slot<u64>,
    /// Text payload injected into stdlib templates for streaming scenarios.
    pub stdlib_text: Slot<String>,

    // HTTP server state (non-Clone)
    /// Last HTTP server fixture started by stdlib steps.
    pub http_server: RefCell<Option<HttpServer>>,
    /// URL exposed by the active HTTP server fixture.
    pub stdlib_url: Slot<String>,

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
        self.restore_environment();
        self.stdlib_text.clear();
    }
}

/// Extract the host component from a URL.
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

/// Strip surrounding double quotes from a string parameter.
///
/// rstest-bdd captures quoted strings including the quotes (unlike cucumber),
/// so we need to strip them when processing step parameters.
#[must_use]
pub fn strip_quotes(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|stripped| stripped.strip_suffix('"'))
        .unwrap_or(s)
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
