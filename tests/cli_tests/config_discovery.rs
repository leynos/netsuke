//! Configuration discovery integration tests.
//!
//! These tests verify automatic configuration file discovery in project and
//! user scopes, environment variable precedence, and CLI flag overrides. The
//! cases are split across `config_discovery_scopes.rs` (project and user
//! scope discovery plus precedence) and `config_discovery_overrides.rs`
//! (environment, CLI flag, and explicit path overrides).

#[path = "config_discovery_overrides.rs"]
mod overrides;
#[path = "config_discovery_scopes.rs"]
mod scopes;
