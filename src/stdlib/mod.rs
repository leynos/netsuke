//! Standard library registration for `MiniJinja` templates.
//!
//! Wires file tests, path helpers, collection utilities, network helpers, and
//! command wrappers into a single entrypoint so templates behave consistently
//! across projects.

mod collections;
mod command;
mod config;
mod network;
mod path;
mod register;
mod time;
mod which;

pub use config::{
    DEFAULT_COMMAND_MAX_OUTPUT_BYTES, DEFAULT_COMMAND_MAX_STREAM_BYTES, DEFAULT_COMMAND_TEMP_DIR,
    DEFAULT_FETCH_CACHE_DIR, DEFAULT_FETCH_MAX_RESPONSE_BYTES, NetworkConfig, StdlibConfig,
};
pub use network::{
    HostPatternError, NetworkPolicy, NetworkPolicyConfigError, NetworkPolicyViolation,
};
pub use register::{register, register_with_config, value_from_bytes};

use std::{
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
};

/// Captures mutable state shared between stdlib helpers.
#[derive(Clone, Default, Debug)]
pub struct StdlibState {
    impure: Arc<AtomicBool>,
}

impl StdlibState {
    /// Returns whether any impure helper executed during the last render.
    #[must_use]
    pub fn is_impure(&self) -> bool {
        self.impure.load(Ordering::Relaxed)
    }

    /// Resets the impurity marker so callers can track helper usage per render.
    pub fn reset_impure(&self) {
        self.impure.store(false, Ordering::Relaxed);
    }

    pub(crate) fn impure_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.impure)
    }
}
