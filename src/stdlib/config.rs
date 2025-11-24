//! Configuration types and defaults for wiring the stdlib into `MiniJinja`.

use super::{command, network::NetworkPolicy};
use anyhow::bail;
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use std::{env, sync::Arc};

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

/// Configuration for registering Netsuke's standard library helpers.
#[derive(Debug, Clone)]
pub struct StdlibConfig {
    workspace_root: Arc<Dir>,
    workspace_root_path: Option<Utf8PathBuf>,
    fetch_cache_relative: Utf8PathBuf,
    network_policy: NetworkPolicy,
    fetch_max_response_bytes: u64,
    command_max_output_bytes: u64,
    command_max_stream_bytes: u64,
}

impl StdlibConfig {
    /// Create a configuration bound to `workspace_root`.
    ///
    /// # Panics
    ///
    /// Panics if Netsuke's built-in fetch cache directory constant fails
    /// validation. This indicates a programming error and should never occur in
    /// production builds.
    #[must_use]
    pub fn new(workspace_root: Dir) -> Self {
        let default = Utf8PathBuf::from(DEFAULT_FETCH_CACHE_DIR);
        // Rationale: the constant is static and validated for defence in depth.
        if let Err(err) = Self::validate_cache_relative(&default) {
            panic!("default fetch cache path should be valid: {err}");
        }
        Self {
            workspace_root: Arc::new(workspace_root),
            workspace_root_path: None,
            fetch_cache_relative: default,
            network_policy: NetworkPolicy::default(),
            fetch_max_response_bytes: DEFAULT_FETCH_MAX_RESPONSE_BYTES,
            command_max_output_bytes: DEFAULT_COMMAND_MAX_OUTPUT_BYTES,
            command_max_stream_bytes: DEFAULT_COMMAND_MAX_STREAM_BYTES,
        }
    }

    /// Record the absolute workspace root path for capability-scoped helpers.
    ///
    /// # Panics
    ///
    /// Panics if `path` is not absolute.
    #[must_use]
    pub fn with_workspace_root_path(mut self, path: impl Into<Utf8PathBuf>) -> Self {
        let absolute = path.into();
        assert!(absolute.is_absolute(), "workspace root must be absolute");
        self.workspace_root_path = Some(absolute);
        self
    }

    /// Override the network cache location relative to the workspace root.
    ///
    /// # Errors
    ///
    /// Returns an error when the path is empty, absolute, or escapes the
    /// workspace via parent components.
    pub fn with_fetch_cache_relative(mut self, relative_path: Utf8PathBuf) -> anyhow::Result<Self> {
        Self::validate_cache_relative(&relative_path)?;
        self.fetch_cache_relative = relative_path;
        Ok(self)
    }

    /// Override the network policy used by stdlib helpers.
    #[must_use]
    pub fn with_network_policy(mut self, policy: NetworkPolicy) -> Self {
        self.network_policy = policy;
        self
    }

    /// Override the maximum size for HTTP responses fetched via stdlib helpers.
    ///
    /// # Errors
    ///
    /// Returns an error when `max_bytes` is zero.
    pub fn with_fetch_max_response_bytes(mut self, max_bytes: u64) -> anyhow::Result<Self> {
        anyhow::ensure!(max_bytes > 0, "fetch response limit must be positive");
        self.fetch_max_response_bytes = max_bytes;
        Ok(self)
    }

    /// Override the maximum captured stdout size for stdlib command helpers.
    ///
    /// # Errors
    ///
    /// Returns an error when `max_bytes` is zero.
    pub fn with_command_max_output_bytes(mut self, max_bytes: u64) -> anyhow::Result<Self> {
        anyhow::ensure!(max_bytes > 0, "command output limit must be positive");
        self.command_max_output_bytes = max_bytes;
        Ok(self)
    }

    /// Override the maximum streamed stdout size for stdlib command helpers.
    ///
    /// # Errors
    ///
    /// Returns an error when `max_bytes` is zero.
    pub fn with_command_max_stream_bytes(mut self, max_bytes: u64) -> anyhow::Result<Self> {
        anyhow::ensure!(max_bytes > 0, "command stream limit must be positive");
        self.command_max_stream_bytes = max_bytes;
        Ok(self)
    }

    /// The configured fetch cache directory relative to the workspace root.
    #[must_use]
    pub fn fetch_cache_relative(&self) -> &Utf8Path {
        &self.fetch_cache_relative
    }

    /// Consume the configuration and expose component modules with owned state.
    pub(crate) fn into_components(self) -> (NetworkConfig, command::CommandConfig) {
        let Self {
            workspace_root,
            workspace_root_path,
            fetch_cache_relative,
            network_policy,
            fetch_max_response_bytes,
            command_max_output_bytes,
            command_max_stream_bytes,
        } = self;

        let command_root = Arc::clone(&workspace_root);
        let network = NetworkConfig {
            cache_root: workspace_root,
            cache_relative: fetch_cache_relative,
            policy: network_policy,
            max_response_bytes: fetch_max_response_bytes,
        };

        let command = command::CommandConfig::new(
            command_max_output_bytes,
            command_max_stream_bytes,
            command_root,
            workspace_root_path.map(Arc::new),
        );

        (network, command)
    }

    pub(crate) fn validate_cache_relative(relative: &Utf8Path) -> anyhow::Result<()> {
        if relative.as_str().is_empty() {
            bail!("fetch cache path must not be empty");
        }

        if relative.is_absolute() {
            bail!(
                "fetch cache path '{}' must be relative to the workspace",
                relative
            );
        }

        for component in relative.components() {
            if matches!(
                component,
                Utf8Component::ParentDir | Utf8Component::Prefix(_)
            ) {
                bail!(
                    "fetch cache path '{}' must stay within the workspace",
                    relative
                );
            }
        }

        Ok(())
    }

    pub(crate) fn workspace_root_path(&self) -> Option<&Utf8Path> {
        self.workspace_root_path.as_deref()
    }
}

impl Default for StdlibConfig {
    fn default() -> Self {
        let root = Dir::open_ambient_dir(".", ambient_authority())
            .unwrap_or_else(|err| panic!("open stdlib workspace root: {err}"));
        let cwd =
            env::current_dir().unwrap_or_else(|err| panic!("resolve current directory: {err}"));
        let path = Utf8PathBuf::from_path_buf(cwd)
            .unwrap_or_else(|path| panic!("cwd contains non-UTF-8 components: {}", path.display()));
        Self::new(root).with_workspace_root_path(path)
    }
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

#[cfg(test)]
mod tests {
    use super::{DEFAULT_COMMAND_MAX_OUTPUT_BYTES, DEFAULT_COMMAND_MAX_STREAM_BYTES, StdlibConfig};
    use camino::{Utf8Path, Utf8PathBuf};
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use std::env;

    #[test]
    fn validate_cache_relative_rejects_empty() {
        let err = StdlibConfig::validate_cache_relative(Utf8Path::new(""))
            .expect_err("empty path should fail");
        assert_eq!(err.to_string(), "fetch cache path must not be empty");
    }

    #[test]
    fn validate_cache_relative_rejects_absolute_paths() {
        let err = StdlibConfig::validate_cache_relative(Utf8Path::new("/cache"))
            .expect_err("absolute path should fail");
        assert_eq!(
            err.to_string(),
            "fetch cache path '/cache' must be relative to the workspace"
        );
    }

    #[test]
    fn validate_cache_relative_rejects_parent_components() {
        let err = StdlibConfig::validate_cache_relative(Utf8Path::new("../escape"))
            .expect_err("parent components should fail");
        assert_eq!(
            err.to_string(),
            "fetch cache path '../escape' must stay within the workspace"
        );
    }

    #[test]
    fn validate_cache_relative_accepts_workspace_relative_paths() {
        StdlibConfig::validate_cache_relative(Utf8Path::new("nested/cache"))
            .expect("relative path should be accepted");
    }

    #[test]
    fn command_limits_default_to_constants() {
        let config = StdlibConfig::default();
        assert_eq!(
            config.command_max_output_bytes,
            DEFAULT_COMMAND_MAX_OUTPUT_BYTES
        );
        assert_eq!(
            config.command_max_stream_bytes,
            DEFAULT_COMMAND_MAX_STREAM_BYTES
        );
    }

    #[test]
    fn command_output_limit_builder_updates_value() {
        let config = StdlibConfig::default()
            .with_command_max_output_bytes(2_048)
            .expect("positive limits should succeed");
        assert_eq!(config.command_max_output_bytes, 2_048);
    }

    #[test]
    fn command_output_limit_builder_rejects_zero() {
        let err = StdlibConfig::default()
            .with_command_max_output_bytes(0)
            .expect_err("zero-byte limits must be rejected");
        assert_eq!(err.to_string(), "command output limit must be positive");
    }

    #[test]
    fn command_stream_limit_builder_updates_value() {
        let config = StdlibConfig::default()
            .with_command_max_stream_bytes(65_536)
            .expect("positive limits should succeed");
        assert_eq!(config.command_max_stream_bytes, 65_536);
    }

    #[test]
    fn command_stream_limit_builder_rejects_zero() {
        let err = StdlibConfig::default()
            .with_command_max_stream_bytes(0)
            .expect_err("zero-byte limits must be rejected");
        assert_eq!(err.to_string(), "command stream limit must be positive");
    }

    #[test]
    fn command_limits_propagate_into_components() {
        let dir = Dir::open_ambient_dir(".", ambient_authority())
            .expect("open workspace root for config tests");
        let path = Utf8PathBuf::from_path_buf(
            env::current_dir().expect("resolve cwd for command config test"),
        )
        .expect("cwd should be valid UTF-8");
        let config = StdlibConfig::new(dir)
            .with_workspace_root_path(path)
            .with_command_max_output_bytes(4_096)
            .expect("set capture limit")
            .with_command_max_stream_bytes(131_072)
            .expect("set streaming limit");
        let (_network, command) = config.into_components();
        assert_eq!(command.max_capture_bytes, 4_096);
        assert_eq!(command.max_stream_bytes, 131_072);
    }
}
