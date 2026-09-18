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
    //!
    //! Fixtures standing in for a special file type — a symlink, a FIFO, a
    //! device — obey one invariant: a special-file policy test must create the
    //! requested file type or skip because that file type is unavailable. It
    //! must not substitute a regular file. A regular-file stand-in silently
    //! changes what the test exercises, so the policy goes unverified while the
    //! test still reports a verdict — and a fixture that hands back a regular
    //! file where a symlink was requested inverts the assertion outright.

    use super::{Workspace, stdlib};
    #[cfg(windows)]
    use anyhow::ensure;
    use anyhow::{Context, Result, anyhow};
    use camino::{Utf8Path, Utf8PathBuf};
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

    /// Build a workspace holding the regular `file` fixture (contents `data`)
    /// and the `lines.txt` fixture.
    ///
    /// No symlink is created here: the workspace deliberately holds only file
    /// types every platform can provide. Callers needing a symlink ask
    /// [`file_symlink_fixture`] for one and honour its availability result.
    pub(crate) fn filter_workspace() -> Result<Workspace> {
        let temp = tempdir().context("create standard filter workspace")?;
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow!("workspace path is not valid UTF-8: {}", path.display()))?;
        let dir = Dir::open_ambient_dir(&root, ambient_authority())
            .context("open filter workspace directory")?;
        dir.write("file", b"data")
            .context("write fixture file 'file'")?;
        dir.write("lines.txt", b"one\ntwo\nthree\n")
            .context("write fixture file 'lines.txt'")?;
        Ok((temp, root))
    }

    /// Create the workspace's real file symlink, `<root>/link` -> `file`, and
    /// report the link's path.
    ///
    /// `Ok(None)` means this host cannot provide a file symlink at all: either
    /// the platform has no symlink support, or Windows refused for want of
    /// `SeCreateSymbolicLinkPrivilege` and Developer Mode — the environmental
    /// `ERROR_PRIVILEGE_NOT_HELD` condition. Every other failure is a genuine
    /// setup fault and propagates.
    ///
    /// Callers skip their symlink-specific assertions on `Ok(None)`. They must
    /// not fall back to a regular file: a special-file policy test must create
    /// the requested file type or skip because that file type is unavailable.
    /// It must not substitute a regular file.
    ///
    /// # Errors
    ///
    /// Returns the setup error when the platform can create symlinks but this
    /// one was not created.
    pub(crate) fn file_symlink_fixture(root: &Utf8Path) -> Result<Option<Utf8PathBuf>> {
        #[cfg(unix)]
        {
            let dir = Dir::open_ambient_dir(root, ambient_authority())
                .context("open filter workspace for the symlink fixture")?;
            dir.symlink("file", "link")
                .context("create fixture symlink 'link' -> 'file'")?;
            Ok(Some(root.join("link")))
        }
        #[cfg(windows)]
        {
            let link = root.join("link");
            match std::os::windows::fs::symlink_file("file", &link) {
                Ok(()) => Ok(Some(link)),
                Err(err) if is_symlink_privilege_error(&err) => Ok(None),
                Err(err) => Err(err).context("create fixture symlink 'link' -> 'file'"),
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = root;
            Ok(None)
        }
    }

    /// Whether `err` is Windows declining a symlink for want of
    /// `SeCreateSymbolicLinkPrivilege` and Developer Mode.
    ///
    /// Only that documented environmental condition reports "symlink
    /// unavailable"; callers surface anything else as a setup failure. Matching
    /// the raw status rather than `PermissionDenied` keeps an ACL denial on the
    /// workspace visible instead of silently skipping the symlink assertions.
    #[cfg(windows)]
    fn is_symlink_privilege_error(err: &std::io::Error) -> bool {
        /// Status `CreateSymbolicLink` reports when neither the privilege nor
        /// Developer Mode is available.
        const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;
        err.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD)
    }

    /// Create the workspace's real directory junction, `<root>/junc` ->
    /// `<root>/junction_target`, and report the junction's path.
    ///
    /// A junction is the directory-shaped reparse point this policy refuses.
    /// Unlike a symlink it needs no privilege and no Developer Mode, so the
    /// fixture is available on every ordinary Windows host; it is built with
    /// `cmd /C mklink /J`, which is the only unprivileged route to one.
    ///
    /// `Ok(None)` means this host has no `cmd.exe`, so the fixture cannot be
    /// created at all. That is the only environmental unavailability this
    /// fixture recognises. Every other failure — including a non-zero `mklink`
    /// exit — is a setup fault and propagates, because junction creation needs
    /// no privilege and reparse points are supported on every filesystem this
    /// suite runs on: skipping there would report green over a policy that went
    /// unexercised.
    ///
    /// Callers assert the entry really carries the reparse-point attribute
    /// before rendering. They must not fall back to a plain directory: a
    /// special-file policy test must create the requested file type or skip
    /// because that file type is unavailable. It must not substitute one.
    ///
    /// # Errors
    ///
    /// Returns the setup error when `cmd` is present but the junction was not
    /// created, quoting `mklink`'s own diagnostics.
    #[cfg(windows)]
    pub(crate) fn junction_fixture(root: &Utf8Path) -> Result<Option<Utf8PathBuf>> {
        use std::os::windows::process::CommandExt as _;
        use std::process::Command;

        /// Name of the junction the fixture creates.
        const LINK: &str = "junc";
        /// Name of the directory the junction points at.
        const TARGET: &str = "junction_target";

        let dir = Dir::open_ambient_dir(root, ambient_authority())
            .context("open filter workspace for the junction fixture")?;
        dir.create_dir(TARGET)
            .context("create the junction fixture target directory")?;
        let link = root.join(LINK);
        let target = root.join(TARGET);
        ensure!(
            !link.as_str().contains('"') && !target.as_str().contains('"'),
            "workspace path contains a quote and cannot be passed to cmd: \
             link {link}, target {target}"
        );

        // `mklink` is a `cmd` built-in, so it is reachable only through
        // `cmd /C`. `raw_arg` passes the command line verbatim because `cmd`
        // parses that line itself, while Rust's standard argument quoting
        // targets the C runtime's rules rather than `cmd`'s.
        let output = match Command::new("cmd")
            .arg("/C")
            .raw_arg(format!(r#"mklink /J "{link}" "{target}""#))
            .output()
        {
            Ok(output) => output,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(err).context("run 'cmd /C mklink /J' for the junction fixture");
            }
        };
        let stderr = String::from_utf8_lossy(&output.stderr);
        ensure!(
            output.status.success(),
            "create junction fixture {link} -> {target}: cmd exited with {}: {}",
            output.status,
            stderr.trim()
        );
        Ok(Some(link))
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
