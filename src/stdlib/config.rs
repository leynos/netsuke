//! Configuration types and defaults for wiring the stdlib into `MiniJinja`.

use super::config_types::HomeDirectory;
pub use super::config_types::{
    DEFAULT_COMMAND_MAX_OUTPUT_BYTES, DEFAULT_COMMAND_MAX_STREAM_BYTES, DEFAULT_COMMAND_TEMP_DIR,
    DEFAULT_FETCH_CACHE_DIR, DEFAULT_FETCH_MAX_RESPONSE_BYTES, DEFAULT_WHICH_CACHE_CAPACITY,
    NetworkConfig,
};
use super::{command, network::NetworkPolicy, which::WORKSPACE_SKIP_DIRS};
use crate::localization::{self, keys};
use anyhow::{Context, anyhow, bail, ensure};
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use indexmap::IndexSet;
use std::{env, ffi::OsString, num::NonZeroUsize, sync::Arc};

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

    /// Override the cache capacity for the `which` resolver.
    ///
    /// # Errors
    ///
    /// Returns an error when `capacity` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// # use cap_std::{ambient_authority, fs_utf8::Dir};
    /// # use netsuke::stdlib::StdlibConfig;
    /// let dir = Dir::open_ambient_dir(".", ambient_authority())
    ///     .expect("open ambient workspace");
    /// let _config = StdlibConfig::new(dir)
    ///     .expect("construct stdlib config")
    ///     .with_which_cache_capacity(128)
    ///     .expect("set which cache capacity");
    /// // Config can now be passed to stdlib registration with a larger cache.
    /// ```
    pub fn with_which_cache_capacity(mut self, capacity: usize) -> anyhow::Result<Self> {
        let non_zero_capacity = NonZeroUsize::new(capacity).ok_or_else(|| {
            anyhow!(
                "{}",
                localization::message(keys::STDLIB_WHICH_CACHE_CAPACITY_POSITIVE)
            )
        })?;
        self.which_cache_capacity = non_zero_capacity;
        Ok(self)
    }
    /// Override the workspace directories skipped by the `which` fallback
    /// search to avoid expensive scans.
    ///
    /// # Errors
    ///
    /// Returns an error when any entry is empty, navigates (for example `..`),
    /// or contains path separators, because skip entries operate on directory
    /// basenames.
    pub fn with_workspace_skip_dirs<I, S>(mut self, dirs: I) -> anyhow::Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut validated = IndexSet::new();
        for dir in dirs {
            let candidate = dir.as_ref().trim();
            ensure!(
                !candidate.is_empty(),
                "{}",
                localization::message(keys::STDLIB_SKIP_DIR_EMPTY)
            );
            ensure!(
                !matches!(candidate, "." | ".."),
                "{}",
                localization::message(keys::STDLIB_SKIP_DIR_NAVIGATION)
            );
            ensure!(
                !candidate.contains(['/', '\\']),
                "{}",
                localization::message(keys::STDLIB_SKIP_DIR_SEPARATOR)
            );
            validated.insert(candidate.to_owned());
        }
        self.workspace_skip_dirs = validated.into_iter().collect();
        Ok(self)
    }

    /// Override the `PATH` environment variable for `which` lookups.
    ///
    /// When set, the stdlib will use the provided path string instead of
    /// reading `PATH` from the process environment. This allows test isolation
    /// without mutating global state.
    #[must_use]
    pub fn with_path_override(mut self, path: impl Into<OsString>) -> Self {
        self.path_override = Some(path.into());
        self
    }

    /// Return the configured PATH override, if any.
    pub(crate) const fn path_override(&self) -> Option<&OsString> {
        self.path_override.as_ref()
    }

    /// Override the `PATH` supplied to child processes run by command filters.
    ///
    /// This seam is intended for callers that need deterministic command
    /// resolution without mutating the process-wide environment.
    #[must_use]
    pub fn with_command_path_override(mut self, path: impl Into<OsString>) -> Self {
        self.command_path_override = Some(path.into());
        self
    }

    /// Override home-directory discovery for the `expanduser` filter.
    ///
    /// `Some(path)` supplies a home directory; `None` models a host without
    /// one. Without this override, registration uses the process environment.
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

    /// Directories skipped during `which` workspace fallback scans.
    #[must_use]
    pub fn workspace_skip_dirs(&self) -> &[String] {
        &self.workspace_skip_dirs
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

    pub(crate) const fn which_cache_capacity(&self) -> NonZeroUsize {
        self.which_cache_capacity
    }
}

impl StdlibConfig {
    /// Construct a configuration rooted at the ambient current directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the workspace root cannot be opened with
    /// capability-based I/O, when the current directory cannot be resolved,
    /// or when the current directory contains non-UTF-8 components.
    ///
    /// # Examples
    ///
    /// ```
    /// # use netsuke::stdlib::StdlibConfig;
    /// let config = StdlibConfig::from_current_dir().expect("open workspace at cwd");
    /// // The configuration is rooted at the process working directory.
    /// ```
    pub fn from_current_dir() -> anyhow::Result<Self> {
        let root = Dir::open_ambient_dir(".", ambient_authority()).context(
            localization::message(keys::STDLIB_CONFIG_OPEN_WORKSPACE_ROOT),
        )?;
        let cwd =
            env::current_dir().context(localization::message(keys::STDLIB_CONFIG_RESOLVE_CWD))?;
        let path = Utf8PathBuf::from_path_buf(cwd).map_err(|path| {
            anyhow!(
                "{}",
                localization::message(keys::STDLIB_CONFIG_CWD_NON_UTF8)
                    .with_arg("path", path.display().to_string())
            )
        })?;
        tracing::debug!(path = %path, "resolved stdlib workspace root from current directory");
        Self::new(root)
            .context("default fetch cache path should be valid")?
            .with_workspace_root_path(path)
            .context("workspace root must be absolute")
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
