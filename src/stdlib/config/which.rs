//! `which` resolver configuration on [`StdlibConfig`].
//!
//! The builders and accessors governing executable resolution live together
//! here — cache capacity, workspace skip list, and the `PATH`/`PATHEXT`
//! overrides that let a caller pin the whole search without touching the
//! process environment. Grouping them by feature keeps `config/mod.rs` to the
//! shared configuration surface rather than one module per layer.

use std::{ffi::OsString, num::NonZeroUsize};

use anyhow::{anyhow, ensure};
use indexmap::IndexSet;

use super::StdlibConfig;
use crate::localization::{self, keys};

impl StdlibConfig {
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

    /// Override the `PATHEXT` environment variable for `which` lookups.
    ///
    /// The counterpart to [`Self::with_path_override`] for the second variable
    /// Windows executable resolution depends upon, so a caller can pin the
    /// whole search — directories *and* extensions — without touching the
    /// process environment. `PATHEXT` is meaningless elsewhere, so the
    /// override is accepted and ignored off Windows.
    ///
    /// An empty or whitespace-only value is not an empty extension list: it
    /// yields the built-in fallback, because a genuinely empty list would
    /// match nothing.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use minijinja::Environment;
    /// use netsuke::stdlib::{self, StdlibConfig};
    /// let config = StdlibConfig::from_current_dir()
    ///     .expect("open workspace")
    ///     .with_pathext_override(".com;.exe");
    /// let mut env = Environment::new();
    /// stdlib::register_with_config(&mut env, config).expect("register stdlib");
    /// // On Windows `which('cargo')` now considers only `.com` and `.exe`;
    /// // a `cargo.bat` would no longer be a candidate.
    /// ```
    #[must_use]
    pub fn with_pathext_override(mut self, pathext: impl Into<OsString>) -> Self {
        self.pathext_override = Some(pathext.into());
        self
    }

    /// Return the configured PATHEXT override, if any.
    pub(crate) const fn pathext_override(&self) -> Option<&OsString> {
        self.pathext_override.as_ref()
    }

    /// Directories skipped during `which` workspace fallback scans.
    #[must_use]
    pub fn workspace_skip_dirs(&self) -> &[String] {
        &self.workspace_skip_dirs
    }

    /// Return the configured `which` cache capacity.
    pub(crate) const fn which_cache_capacity(&self) -> NonZeroUsize {
        self.which_cache_capacity
    }
}
