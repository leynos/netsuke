//! Standard library registration for `MiniJinja` templates.
//!
//! The module wires the platform-aware file tests, the path manipulation
//! filters, the collection helpers, the network utilities, and the command
//! wrappers into a single entrypoint so template authors can rely on
//! consistent behaviour across projects. Tests such as `dir`, `file`, and
//! `symlink` inspect metadata without following symlinks, while filters
//! expose conveniences like `basename`, `with_suffix`, `realpath`, content
//! hashing, collection utilities including `flatten`, `group_by`, and `uniq`,
//! HTTP helpers like `fetch`, and shell bridges such as `shell` and `grep`.

mod collections;
mod command;
mod network;
mod path;
mod time;
mod which;

pub use network::{
    HostPatternError, NetworkPolicy, NetworkPolicyConfigError, NetworkPolicyViolation,
};

use anyhow::{Context, bail};
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
#[cfg(unix)]
use cap_std::fs::FileTypeExt;
use cap_std::{ambient_authority, fs, fs_utf8::Dir};
use minijinja::{Environment, Error, value::Value};
use std::{
    env,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
};

type FileTest = (&'static str, fn(fs::FileType) -> bool);

/// Default relative path for the fetch cache within the workspace.
pub(crate) const DEFAULT_FETCH_CACHE_DIR: &str = ".netsuke/fetch";
/// Default upper bound for network helper responses (8 MiB).
pub(crate) const DEFAULT_FETCH_MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;
/// Default upper bound for captured command output (1 MiB).
pub(crate) const DEFAULT_COMMAND_MAX_OUTPUT_BYTES: u64 = 1024 * 1024;
/// Default upper bound for streamed command output files (64 MiB).
pub(crate) const DEFAULT_COMMAND_MAX_STREAM_BYTES: u64 = 64 * 1024 * 1024;
/// Relative directory for command helper tempfiles.
pub(crate) const DEFAULT_COMMAND_TEMP_DIR: &str = ".netsuke/tmp";

/// Configuration for registering Netsuke's standard library helpers.
///
/// The configuration records the capability-scoped workspace directory used to
/// sandbox helper I/O and the relative path where network caches are stored.
///
/// # Examples
///
/// ```rust,no_run
/// use cap_std::{ambient_authority, fs_utf8::Dir};
/// use minijinja::Environment;
/// use netsuke::stdlib::{self, StdlibConfig};
///
/// let root = Dir::open_ambient_dir(".", ambient_authority())
///     .expect("open workspace");
/// let mut env = Environment::new();
/// let _state = stdlib::register_with_config(&mut env, StdlibConfig::new(root));
/// ```
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
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cap_std::{ambient_authority, fs_utf8::Dir};
    /// use netsuke::stdlib::StdlibConfig;
    ///
    /// let dir = Dir::open_ambient_dir(".", ambient_authority())
    ///     .expect("open workspace");
    /// let config = StdlibConfig::new(dir);
    /// assert_eq!(config.fetch_cache_relative().as_str(), ".netsuke/fetch");
    /// ```
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
        let workspace_path = path.into();
        assert!(
            workspace_path.is_absolute(),
            "with_workspace_root_path requires an absolute path, got: {workspace_path}"
        );
        self.workspace_root_path = Some(workspace_path);
        self
    }

    /// Return the recorded absolute workspace root path, when available.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use cap_std::{ambient_authority, fs_utf8::Dir};
    /// # use netsuke::stdlib::StdlibConfig;
    /// let root = Dir::open_ambient_dir(".", ambient_authority()).expect("open workspace");
    /// let config = StdlibConfig::new(root).with_workspace_root_path("/tmp/example".into());
    /// assert_eq!(
    ///     config.workspace_root_path().unwrap().as_str(),
    ///     "/tmp/example"
    /// );
    /// ```
    #[must_use]
    pub fn workspace_root_path(&self) -> Option<&Utf8Path> {
        self.workspace_root_path.as_deref()
    }

    /// Override the relative cache directory within the workspace.
    ///
    /// # Errors
    ///
    /// Returns an error when the provided path is empty, absolute, or attempts
    /// to escape the workspace via parent components.
    pub fn with_fetch_cache_relative(
        mut self,
        relative: impl Into<Utf8PathBuf>,
    ) -> anyhow::Result<Self> {
        let relative_path = relative.into();
        Self::validate_cache_relative(&relative_path)?;
        self.fetch_cache_relative = relative_path;
        Ok(self)
    }

    /// Replace the default network policy with a custom configuration.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cap_std::{ambient_authority, fs_utf8::Dir};
    /// use netsuke::stdlib::{NetworkPolicy, StdlibConfig};
    ///
    /// let dir = Dir::open_ambient_dir(".", ambient_authority())
    ///     .expect("open workspace");
    /// let policy = NetworkPolicy::default()
    ///     .allow_scheme("http")
    ///     .expect("allow http");
    /// let config = StdlibConfig::new(dir).with_network_policy(policy);
    /// assert_eq!(config.fetch_cache_relative().as_str(), ".netsuke/fetch");
    /// ```
    #[must_use]
    pub fn with_network_policy(mut self, policy: NetworkPolicy) -> Self {
        self.network_policy = policy;
        self
    }

    /// Override the maximum fetch response size in bytes.
    ///
    /// The limit protects renderers from unbounded downloads. Values must be
    /// strictly positive; zero-byte responses remain permitted because they do
    /// not consume the budget.
    ///
    /// # Errors
    ///
    /// Returns an error when `max_bytes` is zero.
    pub fn with_fetch_max_response_bytes(mut self, max_bytes: u64) -> anyhow::Result<Self> {
        anyhow::ensure!(max_bytes > 0, "fetch response limit must be positive");
        self.fetch_max_response_bytes = max_bytes;
        Ok(self)
    }

    /// Override the maximum captured command output size in bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when `max_bytes` is zero.
    pub fn with_command_max_output_bytes(mut self, max_bytes: u64) -> anyhow::Result<Self> {
        anyhow::ensure!(max_bytes > 0, "command output limit must be positive");
        self.command_max_output_bytes = max_bytes;
        Ok(self)
    }

    /// Override the maximum streamed command output size in bytes.
    ///
    /// Streaming still enforces a ceiling to prevent helpers from exhausting
    /// disk space. Configure the limit according to the largest expected
    /// helper output.
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
pub(crate) struct NetworkConfig {
    pub(crate) cache_root: Arc<Dir>,
    pub(crate) cache_relative: Utf8PathBuf,
    pub(crate) policy: NetworkPolicy,
    pub(crate) max_response_bytes: u64,
}

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

/// Register standard library helpers with the `MiniJinja` environment.
///
/// # Examples
/// ```
/// use minijinja::{context, Environment};
/// use netsuke::stdlib;
///
/// let mut env = Environment::new();
/// let _state = stdlib::register(&mut env).expect("register stdlib");
/// env.add_template("t", "{{ path | basename }}").expect("add template");
/// let tmpl = env.get_template("t").expect("get template");
/// let rendered = tmpl
///     .render(context!(path => "foo/bar.txt"))
///     .expect("render");
/// assert_eq!(rendered, "bar.txt");
/// ```
///
/// # Errors
///
/// Returns an error when the current working directory cannot be opened using
/// capability-based I/O. This occurs when the process lacks permission to read
/// the directory or if it no longer exists.
pub fn register(env: &mut Environment<'_>) -> anyhow::Result<StdlibState> {
    let root = Dir::open_ambient_dir(".", ambient_authority())
        .context("open current directory for stdlib registration")?;
    let cwd = env::current_dir().context("resolve current directory for stdlib registration")?;
    let path = Utf8PathBuf::from_path_buf(cwd).map_err(|path| {
        anyhow::anyhow!("current directory contains non-UTF-8 components: {path:?}")
    })?;
    register_with_config(env, StdlibConfig::new(root).with_workspace_root_path(path))
}

/// Register stdlib helpers using an explicit configuration.
///
/// This is intended for callers that have already derived a capability-scoped
/// workspace directory and need to wire the stdlib into a `MiniJinja`
/// environment.
///
/// # Examples
///
/// ```rust,no_run
/// use cap_std::{ambient_authority, fs_utf8::Dir};
/// use minijinja::Environment;
/// use netsuke::stdlib::{self, StdlibConfig};
///
/// let dir = Dir::open_ambient_dir(".", ambient_authority())
///     .expect("open workspace");
/// let mut env = Environment::new();
/// let _state = stdlib::register_with_config(&mut env, StdlibConfig::new(dir));
/// ```
///
/// # Errors
///
/// Returns an error if stdlib components cannot be registered (for example,
/// when the which resolver cache configuration is invalid).
pub fn register_with_config(
    env: &mut Environment<'_>,
    config: StdlibConfig,
) -> anyhow::Result<StdlibState> {
    let state = StdlibState::default();
    register_file_tests(env);
    path::register_filters(env);
    collections::register_filters(env);
    let which_cwd = config
        .workspace_root_path()
        .map(|path| Arc::new(path.to_path_buf()));
    which::register(env, which_cwd)?;
    let impure = state.impure_flag();
    let (network_config, command_config) = config.into_components();
    network::register_functions(env, Arc::clone(&impure), network_config);
    command::register(env, impure, command_config);
    time::register_functions(env);
    Ok(state)
}

pub(crate) fn value_from_bytes(bytes: Vec<u8>) -> Value {
    match String::from_utf8(bytes) {
        Ok(text) => Value::from(text),
        Err(err) => Value::from_bytes(err.into_bytes()),
    }
}

fn register_file_tests(env: &mut Environment<'_>) {
    const TESTS: &[FileTest] = &[
        ("dir", is_dir),
        ("file", is_file),
        ("symlink", is_symlink),
        ("pipe", is_fifo),
        ("block_device", is_block_device),
        ("char_device", is_char_device),
        ("device", is_device),
    ];

    for &(name, pred) in TESTS {
        env.add_test(name, move |val: Value| -> Result<bool, Error> {
            if let Some(s) = val.as_str() {
                return path::file_type_matches(Utf8Path::new(s), pred);
            }
            Ok(false)
        });
    }
}

fn is_dir(ft: fs::FileType) -> bool {
    ft.is_dir()
}

fn is_file(ft: fs::FileType) -> bool {
    ft.is_file()
}

fn is_symlink(ft: fs::FileType) -> bool {
    ft.is_symlink()
}

#[cfg(unix)]
fn is_fifo(ft: fs::FileType) -> bool {
    ft.is_fifo()
}

#[cfg(not(unix))]
fn is_fifo(_ft: fs::FileType) -> bool {
    false
}

#[cfg(unix)]
fn is_block_device(ft: fs::FileType) -> bool {
    ft.is_block_device()
}

#[cfg(not(unix))]
fn is_block_device(_ft: fs::FileType) -> bool {
    false
}

#[cfg(unix)]
fn is_char_device(ft: fs::FileType) -> bool {
    ft.is_char_device()
}

#[cfg(not(unix))]
fn is_char_device(_ft: fs::FileType) -> bool {
    false
}

#[cfg(unix)]
fn is_device(ft: fs::FileType) -> bool {
    is_block_device(ft) || is_char_device(ft)
}

#[cfg(not(unix))]
fn is_device(_ft: fs::FileType) -> bool {
    false
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
