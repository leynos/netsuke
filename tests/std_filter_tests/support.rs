//! Shared helpers for stdlib filter integration tests.
//!
//! These utilities prepare temporary workspaces and template environments so
//! behaviour-driven scenarios can focus on assertions instead of boilerplate.
//! Helpers expose explicit `StdlibConfig` wiring that binds caches to the
//! isolated workspace used by each test.

use camino::Utf8PathBuf;
use netsuke::stdlib;

pub(crate) type Workspace = (tempfile::TempDir, Utf8PathBuf);

pub(crate) mod fallible {
    //! Fallible fixture builders that preserve setup diagnostics for callers.

    use super::{Workspace, stdlib};
    use anyhow::{Context, Result, anyhow};
    use camino::Utf8PathBuf;
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use minijinja::{Environment, context};
    use netsuke::stdlib::{StdlibConfig, StdlibState};
    use tempfile::tempdir;

    pub(crate) fn register_template(
        env: &mut Environment<'_>,
        name: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<()> {
        let template_name = name.into();
        let template_source = source.into();
        env.add_template_owned(template_name.clone(), template_source)
            .with_context(|| format!("register template '{template_name}'"))?;
        Ok(())
    }

    pub(crate) fn stdlib_env_with_config(
        config: StdlibConfig,
    ) -> Result<(Environment<'static>, StdlibState)> {
        let mut env = Environment::new();
        let state = stdlib::register_with_config(&mut env, config)?;
        Ok((env, state))
    }

    /// Builds a stdlib environment rooted at `root` whose file-read budget is
    /// `limit` bytes.
    pub(crate) fn stdlib_env_with_root_and_file_read_limit(
        root: &camino::Utf8Path,
        limit: u64,
    ) -> Result<Environment<'static>> {
        let dir = Dir::open_ambient_dir(root, ambient_authority())
            .context("open policy workspace root")?;
        let (env, _) = stdlib_env_with_config(
            StdlibConfig::new(dir)?
                .with_workspace_root_path(root)?
                .with_file_max_read_bytes(limit)?,
        )?;
        Ok(env)
    }

    pub(crate) fn stdlib_env_with_state() -> Result<(Environment<'static>, StdlibState)> {
        stdlib_env_with_config(StdlibConfig::from_current_dir()?)
    }

    pub(crate) fn stdlib_env_with_path(
        path: std::ffi::OsString,
    ) -> Result<(Environment<'static>, StdlibState)> {
        stdlib_env_with_config(StdlibConfig::from_current_dir()?.with_path_override(path))
    }

    pub(crate) fn stdlib_env_with_home(
        root: &camino::Utf8Path,
        home: Option<String>,
    ) -> Result<Environment<'static>> {
        let dir = Dir::open_ambient_dir(root, ambient_authority())
            .context("open filter workspace for home override")?;
        let config = StdlibConfig::new(dir)?
            .with_workspace_root_path(root)?
            .with_home_override(home);
        stdlib_env_with_config(config).map(|(env, _)| env)
    }

    pub(crate) fn stdlib_env() -> Result<Environment<'static>> {
        stdlib_env_with_state().map(|(env, _)| env)
    }

    pub(crate) fn filter_workspace() -> Result<Workspace> {
        let temp = tempdir().context("create standard filter workspace")?;
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow!("workspace path is not valid UTF-8: {}", path.display()))?;
        let dir = Dir::open_ambient_dir(&root, ambient_authority())
            .context("open filter workspace directory")?;
        dir.write("file", b"data")
            .context("write fixture file 'file'")?;
        #[cfg(unix)]
        dir.symlink("file", "link")
            .context("create fixture symlink")?;
        #[cfg(not(unix))]
        dir.write("link", b"data")
            .context("create fixture link copy")?;
        dir.write("lines.txt", b"one\ntwo\nthree\n")
            .context("write fixture file 'lines.txt'")?;
        Ok((temp, root))
    }

    pub(crate) fn render<'a>(
        env: &mut Environment<'a>,
        name: &'a str,
        template: &'a str,
        path: &Utf8PathBuf,
    ) -> Result<String> {
        env.add_template(name, template)
            .with_context(|| format!("register template '{name}'"))?;
        let registered_template = env
            .get_template(name)
            .with_context(|| format!("fetch template '{name}'"))?;
        registered_template
            .render(context!(path => path.as_str()))
            .with_context(|| format!("render template '{name}'"))
    }
}

pub(crate) use fallible::filter_workspace;
