//! Configuration types and defaults for wiring the stdlib into `MiniJinja`.

use super::{command, network::NetworkPolicy};
use anyhow::{anyhow, bail, ensure};
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use std::{env, num::NonZeroUsize, sync::Arc};

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
        Self::validate_cache_relative(&default)
            .map_err(|err| anyhow!("default fetch cache path should be valid: {err}"))?;
        let which_cache_capacity = NonZeroUsize::new(DEFAULT_WHICH_CACHE_CAPACITY)
            .ok_or_else(|| anyhow!("default which cache capacity should be positive"))?;
        Ok(Self {
            workspace_root: Arc::new(workspace_root),
            workspace_root_path: None,
            fetch_cache_relative: default,
            network_policy: NetworkPolicy::default(),
            fetch_max_response_bytes: DEFAULT_FETCH_MAX_RESPONSE_BYTES,
            command_max_output_bytes: DEFAULT_COMMAND_MAX_OUTPUT_BYTES,
            command_max_stream_bytes: DEFAULT_COMMAND_MAX_STREAM_BYTES,
            which_cache_capacity,
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
        ensure!(absolute.is_absolute(), "workspace root must be absolute");
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
        ensure!(max_bytes > 0, "fetch response limit must be positive");
        self.fetch_max_response_bytes = max_bytes;
        Ok(self)
    }

    /// Override the maximum captured stdout size for stdlib command helpers.
    ///
    /// # Errors
    ///
    /// Returns an error when `max_bytes` is zero.
    pub fn with_command_max_output_bytes(mut self, max_bytes: u64) -> anyhow::Result<Self> {
        ensure!(max_bytes > 0, "command output limit must be positive");
        self.command_max_output_bytes = max_bytes;
        Ok(self)
    }

    /// Override the maximum streamed stdout size for stdlib command helpers.
    ///
    /// # Errors
    ///
    /// Returns an error when `max_bytes` is zero.
    pub fn with_command_max_stream_bytes(mut self, max_bytes: u64) -> anyhow::Result<Self> {
        ensure!(max_bytes > 0, "command stream limit must be positive");
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
    /// let dir = Dir::open_ambient_dir(".", ambient_authority()).unwrap();
    /// let config = StdlibConfig::new(dir)
    ///     .unwrap()
    ///     .with_which_cache_capacity(128)
    ///     .unwrap();
    /// assert_eq!(config.which_cache_capacity().get(), 128);
    /// ```
    pub fn with_which_cache_capacity(mut self, capacity: usize) -> anyhow::Result<Self> {
        let non_zero_capacity = NonZeroUsize::new(capacity)
            .ok_or_else(|| anyhow!("which cache capacity must be positive"))?;
        self.which_cache_capacity = non_zero_capacity;
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
            ..
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

    pub(crate) const fn which_cache_capacity(&self) -> NonZeroUsize {
        self.which_cache_capacity
    }
}

impl Default for StdlibConfig {
    /// Construct a configuration rooted at the ambient current directory.
    ///
    /// # Panics
    ///
    /// Panics when the workspace root cannot be opened with capability-based
    /// I/O, when the current directory cannot be resolved, or when the current
    /// directory contains non-UTF-8 components. Call [`StdlibConfig::new`]
    /// instead when you need an error-returning API.
    fn default() -> Self {
        let root = Dir::open_ambient_dir(".", ambient_authority())
            .unwrap_or_else(|err| panic!("open stdlib workspace root: {err}"));
        let cwd =
            env::current_dir().unwrap_or_else(|err| panic!("resolve current directory: {err}"));
        let path = Utf8PathBuf::from_path_buf(cwd)
            .unwrap_or_else(|path| panic!("cwd contains non-UTF-8 components: {}", path.display()));
        Self::new(root)
            .unwrap_or_else(|err| panic!("default fetch cache path should be valid: {err}"))
            .with_workspace_root_path(path)
            .unwrap_or_else(|err| panic!("workspace root must be absolute: {err}"))
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
    use super::{
        DEFAULT_COMMAND_MAX_OUTPUT_BYTES, DEFAULT_COMMAND_MAX_STREAM_BYTES,
        DEFAULT_WHICH_CACHE_CAPACITY, StdlibConfig,
    };
    use camino::{Utf8Path, Utf8PathBuf};
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use rstest::{fixture, rstest};
    use std::env;

    #[fixture]
    fn workspace() -> (Dir, Utf8PathBuf) {
        let dir =
            Dir::open_ambient_dir(".", ambient_authority()).expect("open workspace root fixture");
        let path = Utf8PathBuf::from_path_buf(
            env::current_dir().expect("resolve cwd for workspace fixture"),
        )
        .expect("cwd should be valid UTF-8");
        (dir, path)
    }

    #[fixture]
    fn base_config(#[from(workspace)] workspace: (Dir, Utf8PathBuf)) -> StdlibConfig {
        let (dir, path) = workspace;
        StdlibConfig::new(dir)
            .expect("construct stdlib config")
            .with_workspace_root_path(path)
            .expect("record workspace root")
    }

    #[rstest]
    #[case(Utf8Path::new(""), "fetch cache path must not be empty")]
    #[case(
        Utf8Path::new("/cache"),
        "fetch cache path '/cache' must be relative to the workspace"
    )]
    #[case(
        Utf8Path::new("../escape"),
        "fetch cache path '../escape' must stay within the workspace"
    )]
    fn validate_cache_relative_rejects_invalid_inputs(
        #[case] path: &Utf8Path,
        #[case] message: &str,
    ) {
        let err =
            StdlibConfig::validate_cache_relative(path).expect_err("invalid paths should fail");
        assert_eq!(err.to_string(), message);
    }

    #[rstest]
    fn validate_cache_relative_accepts_workspace_relative_paths() {
        StdlibConfig::validate_cache_relative(Utf8Path::new("nested/cache"))
            .expect("relative path should be accepted");
    }

    #[rstest]
    #[case::output(CommandLimitCase {
        builder: StdlibConfig::with_command_max_output_bytes,
        accessor: |cfg: &StdlibConfig| cfg.command_max_output_bytes,
        default_value: DEFAULT_COMMAND_MAX_OUTPUT_BYTES,
        updated: 2_048,
        zero_err: "command output limit must be positive",
    })]
    #[case::stream(CommandLimitCase {
        builder: StdlibConfig::with_command_max_stream_bytes,
        accessor: |cfg: &StdlibConfig| cfg.command_max_stream_bytes,
        default_value: DEFAULT_COMMAND_MAX_STREAM_BYTES,
        updated: 65_536,
        zero_err: "command stream limit must be positive",
    })]
    fn command_limit_builders_validate_and_update(
        base_config: StdlibConfig,
        #[case] case: CommandLimitCase,
    ) {
        assert_eq!((case.accessor)(&base_config), case.default_value);

        let updated_config =
            (case.builder)(base_config.clone(), case.updated).expect("positive limit");
        assert_eq!((case.accessor)(&updated_config), case.updated);

        let err = (case.builder)(base_config, 0).expect_err("zero-byte limits must be rejected");
        assert_eq!(err.to_string(), case.zero_err);
    }

    struct CommandLimitCase {
        builder: fn(StdlibConfig, u64) -> anyhow::Result<StdlibConfig>,
        accessor: fn(&StdlibConfig) -> u64,
        default_value: u64,
        updated: u64,
        zero_err: &'static str,
    }

    #[rstest]
    fn command_limits_propagate_into_components(base_config: StdlibConfig) {
        let config = base_config
            .with_command_max_output_bytes(4_096)
            .expect("set capture limit")
            .with_command_max_stream_bytes(131_072)
            .expect("set streaming limit");
        let (_network, command) = config.into_components();
        assert_eq!(command.max_capture_bytes, 4_096);
        assert_eq!(command.max_stream_bytes, 131_072);
    }

    #[rstest]
    fn which_cache_capacity_validates_and_updates(base_config: StdlibConfig) {
        assert_eq!(
            base_config.which_cache_capacity().get(),
            DEFAULT_WHICH_CACHE_CAPACITY
        );

        let updated = base_config
            .clone()
            .with_which_cache_capacity(5)
            .expect("positive capacity should be accepted");
        assert_eq!(updated.which_cache_capacity().get(), 5);

        let err = base_config
            .with_which_cache_capacity(0)
            .expect_err("zero capacity must be rejected");
        assert_eq!(err.to_string(), "which cache capacity must be positive");
    }
}
