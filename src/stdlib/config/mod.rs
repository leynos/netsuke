//! Configuration types and defaults for wiring the stdlib into `MiniJinja`.

mod ambient;
mod which;

use super::config_types::HomeDirectory;
pub use super::config_types::{
    DEFAULT_COMMAND_MAX_OUTPUT_BYTES, DEFAULT_COMMAND_MAX_STREAM_BYTES, DEFAULT_COMMAND_TEMP_DIR,
    DEFAULT_FETCH_CACHE_DIR, DEFAULT_FETCH_MAX_RESPONSE_BYTES, DEFAULT_FILE_MAX_READ_BYTES,
    DEFAULT_WHICH_CACHE_CAPACITY, FileConfig, NetworkConfig,
};
use super::{command, network::NetworkPolicy, which::WORKSPACE_SKIP_DIRS};
use crate::localization::{self, keys};
use anyhow::{anyhow, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use std::{ffi::OsString, num::NonZeroUsize, sync::Arc};

/// Configuration for registering Netsuke's standard library helpers.
#[derive(Debug, Clone)]
pub struct StdlibConfig {
    /// Capability-scoped handle to the workspace root shared by helpers.
    workspace_root: Arc<Dir>,
    /// Optional absolute UTF-8 workspace path for host-side filesystem access.
    workspace_root_path: Option<Utf8PathBuf>,
    /// Cache directory for network fetches, relative to the workspace root.
    fetch_cache_relative: Utf8PathBuf,
    /// Policy governing which network operations helpers may perform.
    network_policy: NetworkPolicy,
    /// Maximum size (in bytes) of HTTP responses fetched by network helpers.
    fetch_max_response_bytes: u64,
    /// Maximum size (in bytes) read by the file-reading path filters.
    file_max_read_bytes: u64,
    /// Maximum captured stdout size (in bytes) for command helpers.
    command_max_output_bytes: u64,
    /// Maximum streamed stdout size (in bytes) for command helpers.
    command_max_stream_bytes: u64,
    /// Capacity of the executable-resolution cache used by `which` helpers.
    which_cache_capacity: NonZeroUsize,
    /// Directory basenames skipped during `which` fallback workspace scans.
    workspace_skip_dirs: Vec<String>,
    /// Optional `PATH` override used in `which` lookups.
    path_override: Option<OsString>,
    /// Optional `PATHEXT` override used in `which` lookups on Windows.
    pathext_override: Option<OsString>,
    /// Optional deterministic `PATH` supplied to spawned command helpers.
    command_path_override: Option<OsString>,
    /// Home directory source used by the `expanduser` filter.
    home_directory: HomeDirectory,
}

impl StdlibConfig {
    /// Create a configuration bound to `workspace_root`.
    ///
    /// # Errors
    ///
    /// Returns an error if the default fetch cache path fails validation. This
    /// indicates a programming error in the baked-in constant rather than a
    /// runtime condition; callers should treat failures as impossible in
    /// normal operation. The constructor itself never panics.
    pub fn new(workspace_root: Dir) -> anyhow::Result<Self> {
        let default = Utf8PathBuf::from(DEFAULT_FETCH_CACHE_DIR);
        // Rationale: the constant is static and validated for defence in depth.
        Self::validate_cache_relative(&default).map_err(|err| {
            anyhow!(
                "{}",
                localization::message(keys::STDLIB_DEFAULT_FETCH_CACHE_INVALID)
                    .with_arg("details", err.to_string())
            )
        })?;
        let which_cache_capacity =
            NonZeroUsize::new(DEFAULT_WHICH_CACHE_CAPACITY).ok_or_else(|| {
                anyhow!(
                    "{}",
                    localization::message(keys::STDLIB_DEFAULT_WHICH_CACHE_INVALID)
                )
            })?;
        Ok(Self {
            workspace_root: Arc::new(workspace_root),
            workspace_root_path: None,
            fetch_cache_relative: default,
            network_policy: NetworkPolicy::default(),
            fetch_max_response_bytes: DEFAULT_FETCH_MAX_RESPONSE_BYTES,
            file_max_read_bytes: DEFAULT_FILE_MAX_READ_BYTES,
            command_max_output_bytes: DEFAULT_COMMAND_MAX_OUTPUT_BYTES,
            command_max_stream_bytes: DEFAULT_COMMAND_MAX_STREAM_BYTES,
            which_cache_capacity,
            workspace_skip_dirs: WORKSPACE_SKIP_DIRS
                .iter()
                .map(|dir| (*dir).to_owned())
                .collect(),
            path_override: None,
            pathext_override: None,
            command_path_override: None,
            home_directory: HomeDirectory::Ambient,
        })
    }

    /// Record the absolute workspace root path for capability-scoped helpers.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` is not absolute. This protects call sites
    /// that derive the workspace from user input rather than assuming only
    /// programmer-provided paths reach this builder.
    pub fn with_workspace_root_path(mut self, path: impl AsRef<Utf8Path>) -> anyhow::Result<Self> {
        let absolute = path.as_ref();
        ensure!(
            absolute.is_absolute(),
            "{}",
            localization::message(keys::STDLIB_WORKSPACE_ROOT_ABSOLUTE)
        );
        self.workspace_root_path = Some(absolute.to_owned());
        Ok(self)
    }

    /// Override the network cache location relative to the workspace root.
    ///
    /// # Errors
    ///
    /// Returns an error when the path is empty, absolute, or escapes the
    /// workspace via parent components.
    pub fn with_fetch_cache_relative(
        mut self,
        relative_path: impl AsRef<Utf8Path>,
    ) -> anyhow::Result<Self> {
        let relative = relative_path.as_ref();
        Self::validate_cache_relative(relative)?;
        self.fetch_cache_relative = relative.to_owned();
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
        ensure!(
            max_bytes > 0,
            "{}",
            localization::message(keys::STDLIB_FETCH_RESPONSE_LIMIT_POSITIVE)
        );
        self.fetch_max_response_bytes = max_bytes;
        Ok(self)
    }

    /// Override the maximum captured stdout size for stdlib command helpers.
    ///
    /// # Errors
    ///
    /// Returns an error when `max_bytes` is zero.
    pub fn with_command_max_output_bytes(mut self, max_bytes: u64) -> anyhow::Result<Self> {
        ensure!(
            max_bytes > 0,
            "{}",
            localization::message(keys::STDLIB_COMMAND_OUTPUT_LIMIT_POSITIVE)
        );
        self.command_max_output_bytes = max_bytes;
        Ok(self)
    }

    /// Override the maximum size in bytes the file-reading filters may read.
    ///
    /// This budget bounds the `contents`, `linecount`, `hash`, and `digest`
    /// filters so a manifest cannot exhaust memory or CPU through an
    /// unexpectedly large input. Raise it for builds that legitimately hash
    /// large artefacts.
    ///
    /// # Errors
    ///
    /// Returns an error when `max_bytes` is zero.
    pub fn with_file_max_read_bytes(mut self, max_bytes: u64) -> anyhow::Result<Self> {
        ensure!(
            max_bytes > 0,
            "{}",
            localization::message(keys::STDLIB_FILE_READ_LIMIT_POSITIVE)
        );
        self.file_max_read_bytes = max_bytes;
        Ok(self)
    }

    /// The configured maximum size in bytes for file-reading filters.
    pub(crate) const fn file_max_read_bytes(&self) -> u64 {
        self.file_max_read_bytes
    }

    /// Override the maximum streamed stdout size for stdlib command helpers.
    ///
    /// # Errors
    ///
    /// Returns an error when `max_bytes` is zero.
    pub fn with_command_max_stream_bytes(mut self, max_bytes: u64) -> anyhow::Result<Self> {
        ensure!(
            max_bytes > 0,
            "{}",
            localization::message(keys::STDLIB_COMMAND_STREAM_LIMIT_POSITIVE)
        );
        self.command_max_stream_bytes = max_bytes;
        Ok(self)
    }

    /// Override the `PATH` supplied to child processes run by command filters.
    ///
    /// This seam is intended for callers that need deterministic command
    /// resolution without mutating the process-wide environment.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use minijinja::Environment;
    /// use netsuke::stdlib::{self, StdlibConfig};
    /// let tools = tempfile::tempdir().expect("create isolated tools directory");
    /// let config = StdlibConfig::from_current_dir()
    ///     .expect("open workspace")
    ///     .with_command_path_override(tools.path().as_os_str());
    /// let mut env = Environment::new();
    /// stdlib::register_with_config(&mut env, config).expect("register stdlib");
    /// // Command filters search only `tools`; this empty directory cannot
    /// // resolve the platform grep utility.
    /// assert!(env.render_str("{{ 'line' | grep('line') }}", ()).is_err());
    /// ```
    #[must_use]
    pub fn with_command_path_override(mut self, path: impl Into<OsString>) -> Self {
        self.command_path_override = Some(path.into());
        self
    }

    /// Override home-directory discovery for the `expanduser` filter.
    ///
    /// `Some(path)` supplies a home directory; `None` models a host without
    /// one. Without this override, registration uses the process environment.
    ///
    /// # Examples
    ///
    /// ```
    /// use minijinja::{Environment, ErrorKind};
    /// use netsuke::stdlib::{self, StdlibConfig};
    /// let mut explicit_env = Environment::new();
    /// let explicit = StdlibConfig::from_current_dir()
    ///     .expect("open workspace")
    ///     .with_home_override(Some("/srv/example".to_owned()));
    /// stdlib::register_with_config(&mut explicit_env, explicit).expect("register stdlib");
    /// let rendered = explicit_env
    ///     .render_str("{{ '~/work' | expanduser }}", ())
    ///     .expect("expand explicit home");
    /// assert_eq!(rendered, "/srv/example/work");
    ///
    /// let mut missing_env = Environment::new();
    /// let missing = StdlibConfig::from_current_dir()
    ///     .expect("open workspace")
    ///     .with_home_override(None);
    /// stdlib::register_with_config(&mut missing_env, missing).expect("register stdlib");
    /// let error = missing_env
    ///     .render_str("{{ '~/work' | expanduser }}", ())
    ///     .expect_err("missing home should reject expansion");
    /// assert_eq!(error.kind(), ErrorKind::InvalidOperation);
    /// ```
    #[must_use]
    pub fn with_home_override(mut self, home: Option<String>) -> Self {
        self.home_directory = home.map_or(HomeDirectory::Missing, HomeDirectory::Explicit);
        self
    }

    /// Return the configured home directory source.
    pub(crate) const fn home_directory(&self) -> &HomeDirectory {
        &self.home_directory
    }

    /// The configured fetch cache directory relative to the workspace root.
    #[must_use]
    pub fn fetch_cache_relative(&self) -> &Utf8Path {
        &self.fetch_cache_relative
    }

    /// Consume the configuration and expose component modules with owned state.
    pub(crate) fn into_components(self) -> (NetworkConfig, FileConfig, command::CommandConfig) {
        let Self {
            workspace_root,
            workspace_root_path,
            fetch_cache_relative,
            network_policy,
            fetch_max_response_bytes,
            file_max_read_bytes,
            command_max_output_bytes,
            command_max_stream_bytes,
            command_path_override,
            ..
        } = self;

        let command_root = Arc::clone(&workspace_root);
        let files = FileConfig {
            max_read_bytes: file_max_read_bytes,
        };
        let network = NetworkConfig {
            cache_root: workspace_root,
            cache_relative: fetch_cache_relative,
            policy: network_policy,
            max_response_bytes: fetch_max_response_bytes,
        };

        let command = command::CommandConfig::new(command::CommandConfigInit {
            max_capture_bytes: command_max_output_bytes,
            max_stream_bytes: command_max_stream_bytes,
            workspace_root: command_root,
            workspace_root_path: workspace_root_path.map(Arc::new),
            command_path_override,
        });

        (network, files, command)
    }

    /// Validate that a cache path is a non-empty relative path which stays
    /// within the workspace.
    ///
    /// # Errors
    ///
    /// Returns an error when the path is empty, absolute, or contains parent
    /// or prefix components.
    pub(crate) fn validate_cache_relative(relative: &Utf8Path) -> anyhow::Result<()> {
        if relative.as_str().is_empty() {
            bail!("{}", localization::message(keys::STDLIB_FETCH_CACHE_EMPTY));
        }

        if relative.is_absolute() {
            bail!(
                "{}",
                localization::message(keys::STDLIB_FETCH_CACHE_NOT_RELATIVE)
                    .with_arg("path", relative.as_str())
            );
        }

        #[cfg(windows)]
        let has_parent_directory = relative
            .as_str()
            .split(['/', '\\'])
            .any(|component| component == "..");
        #[cfg(not(windows))]
        let has_parent_directory = relative
            .as_std_path()
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir));

        let has_rooted_component = relative.as_std_path().components().any(|component| {
            matches!(
                component,
                std::path::Component::Prefix(_) | std::path::Component::RootDir
            )
        });
        if has_rooted_component {
            bail!(
                "{}",
                localization::message(keys::STDLIB_FETCH_CACHE_NOT_RELATIVE)
                    .with_arg("path", relative.as_str())
            );
        }
        if has_parent_directory {
            bail!(
                "{}",
                localization::message(keys::STDLIB_FETCH_CACHE_ESCAPES)
                    .with_arg("path", relative.as_str())
            );
        }

        Ok(())
    }

    /// Return the absolute workspace root path, if one was configured.
    pub(crate) fn workspace_root_path(&self) -> Option<&Utf8Path> {
        self.workspace_root_path.as_deref()
    }
}

#[cfg(test)]
#[path = "../config_tests.rs"]
mod tests;
