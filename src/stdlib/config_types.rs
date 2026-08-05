//! Supporting value types for standard-library configuration.

use std::sync::Arc;

use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;

use super::network::NetworkPolicy;

/// Default relative path for the fetch cache within the workspace.
pub const DEFAULT_FETCH_CACHE_DIR: &str = ".netsuke/fetch";
/// Default upper bound for network helper responses (8 MiB).
pub const DEFAULT_FETCH_MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;
/// Default upper bound for captured command output (1 MiB).
pub const DEFAULT_COMMAND_MAX_OUTPUT_BYTES: u64 = 1024 * 1024;
/// Default upper bound for streamed command output files (64 MiB).
pub const DEFAULT_COMMAND_MAX_STREAM_BYTES: u64 = 64 * 1024 * 1024;
/// Relative directory for command helper tempfiles.
pub const DEFAULT_COMMAND_TEMP_DIR: &str = ".netsuke/tmp";
/// Default capacity for the `which` resolver cache.
pub const DEFAULT_WHICH_CACHE_CAPACITY: usize = 64;

/// Source used to resolve the current user's home directory.
#[derive(Debug, Clone)]
pub(crate) enum HomeDirectory {
    /// Read the home directory from the process environment.
    Ambient,
    /// Model a process without a discoverable home directory.
    Missing,
    /// Use the supplied home directory.
    Explicit(String),
}

/// Internal configuration passed to the network module for fetch cache initialisation.
#[derive(Clone)]
pub struct NetworkConfig {
    /// Capability-scoped workspace root for network caches.
    pub cache_root: Arc<Dir>,
    /// Relative cache directory within the workspace.
    pub cache_relative: Utf8PathBuf,
    /// Network policy applied to fetch helpers.
    pub policy: NetworkPolicy,
    /// Maximum allowed size for HTTP responses.
    pub max_response_bytes: u64,
}
