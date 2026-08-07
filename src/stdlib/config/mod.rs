//! Configuration types and defaults for wiring the stdlib into `MiniJinja`.

mod ambient;
mod which;

use super::config_types::HomeDirectory;
pub use super::config_types::{
    DEFAULT_COMMAND_MAX_OUTPUT_BYTES, DEFAULT_COMMAND_MAX_STREAM_BYTES, DEFAULT_COMMAND_TEMP_DIR,
    DEFAULT_FETCH_CACHE_DIR, DEFAULT_FETCH_MAX_RESPONSE_BYTES, DEFAULT_WHICH_CACHE_CAPACITY,
    NetworkConfig,
};
use super::{command, network::NetworkPolicy, which::WORKSPACE_SKIP_DIRS};
use crate::localization::{self, keys};
use anyhow::{anyhow, bail, ensure};
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use std::{ffi::OsString, num::NonZeroUsize, sync::Arc};

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
    which_cache_capacity: NonZeroUsize,
    workspace_skip_dirs: Vec<String>,
    path_override: Option<OsString>,
    pathext_override: Option<OsString>,
    command_path_override: Option<OsString>,
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

    pub(crate) const fn home_directory(&self) -> &HomeDirectory {
        &self.home_directory
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
            command_path_override,
            ..
        } = self;

        let command_root = Arc::clone(&workspace_root);
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

        (network, command)
    }

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

        for component in relative.components() {
            if matches!(
                component,
                Utf8Component::ParentDir | Utf8Component::Prefix(_)
            ) {
                bail!(
                    "{}",
                    localization::message(keys::STDLIB_FETCH_CACHE_ESCAPES)
                        .with_arg("path", relative.as_str())
                );
            }
        }

        Ok(())
    }

    pub(crate) fn workspace_root_path(&self) -> Option<&Utf8Path> {
        self.workspace_root_path.as_deref()
    }
}

#[cfg(test)]
#[path = "../config_tests.rs"]
mod tests;
