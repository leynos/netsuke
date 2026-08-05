//! Ambient-current-directory construction for standard-library configuration.

use super::StdlibConfig;
use crate::localization::{self, keys};
use anyhow::{Context, anyhow};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use std::env;

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
