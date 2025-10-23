//! Shared helpers for stdlib filter integration tests.
//!
//! These utilities prepare temporary workspaces and template environments so
//! behaviour-driven scenarios can focus on assertions instead of boilerplate.
//! Helpers expose explicit `StdlibConfig` wiring that binds caches to the
//! isolated workspace used by each test.

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, context};
use netsuke::stdlib::{self, StdlibConfig, StdlibState};

pub(crate) use test_support::{EnvVarGuard, env_lock::EnvLock};

pub(crate) type Workspace = (tempfile::TempDir, Utf8PathBuf);

pub(crate) mod fallible {
    use super::{Workspace, stdlib};
    use anyhow::{anyhow, Context, Result};
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
        let state = stdlib::register_with_config(&mut env, config);
        Ok((env, state))
    }

    pub(crate) fn stdlib_env_with_state() -> Result<(Environment<'static>, StdlibState)> {
        stdlib_env_with_config(StdlibConfig::default())
    }

    pub(crate) fn stdlib_env() -> Result<Environment<'static>> {
        stdlib_env_with_state().map(|(env, _)| env)
    }

    pub(crate) fn filter_workspace() -> Result<Workspace> {
        let temp = tempdir().context("create standard filter workspace")?;
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow!("workspace path is not valid UTF-8: {path:?}"))?;
        let dir = Dir::open_ambient_dir(&root, ambient_authority())
            .context("open filter workspace directory")?;
        dir.write("file", b"data").context("write fixture file 'file'")?;
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
        let template = env
            .get_template(name)
            .with_context(|| format!("fetch template '{name}'"))?;
        template
            .render(context!(path => path.as_str()))
            .with_context(|| format!("render template '{name}'"))
    }
}

pub(crate) use fallible::{
    filter_workspace,
    register_template,
    render,
    stdlib_env,
    stdlib_env_with_config,
    stdlib_env_with_state,
};
