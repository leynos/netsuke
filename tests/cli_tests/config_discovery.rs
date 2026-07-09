//! Configuration discovery integration tests.
//!
//! These tests verify automatic configuration file discovery in project and
//! user scopes, environment variable precedence, and CLI flag overrides. The
//! cases are split across `config_discovery_scopes.rs` (project and user
//! scope discovery plus precedence) and `config_discovery_overrides.rs`
//! (environment, CLI flag, and explicit path overrides).

use anyhow::Context;

#[path = "config_discovery_overrides.rs"]
mod overrides;
#[path = "config_discovery_scopes.rs"]
mod scopes;

/// RAII guard that restores the process working directory on drop.
///
/// Acquire this *after* `EnvLock` so the drop order (CWD restored first,
/// lock released second) mirrors the acquire order.
struct CwdGuard(std::path::PathBuf);

impl CwdGuard {
    fn acquire() -> anyhow::Result<Self> {
        Ok(Self(
            std::env::current_dir().context("capture current working directory")?,
        ))
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        drop(std::env::set_current_dir(&self.0));
    }
}
