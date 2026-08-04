//! Host-utility discovery for the hermetic dev-fast sandbox.

use anyhow::{Context, Result, bail};
use camino::Utf8PathBuf;
use mockable::{DefaultEnv, Env};

use crate::fs;

/// Utilities the scripts and `make` legitimately need. Kept explicit so a new
/// dependency surfaces as a test failure rather than silently resolving to
/// whatever the developer happens to have installed.
pub(super) const SANDBOX_UTILITIES: &[&str] = &[
    "awk",
    "bash",
    "cat",
    "chmod",
    "cp",
    "curl",
    "dirname",
    "env",
    "grep",
    "gzip",
    "ln",
    "make",
    "mkdir",
    "mktemp",
    "rm",
    "rmdir",
    "sed",
    "sh",
    "sha256sum",
    "stat",
    "tar",
    "touch",
    "tr",
    "uname",
];

/// The genuine binary behind a sandboxed utility name.
///
/// A fake that must delegate to the real tool needs an absolute path because
/// the sandbox `PATH` names the fake.
///
/// # Errors
///
/// Returns an error if the utility cannot be resolved from the host environment.
pub fn real_utility(utility: &str) -> Result<Utf8PathBuf> {
    real_utility_with_env(&DefaultEnv, utility)
}

/// Resolve a genuine utility using an injected environment.
///
/// This is the test seam for [`real_utility`]; it is not a general-purpose
/// executable discovery API.
///
/// # Errors
///
/// Returns an error if the utility cannot be resolved from the supplied environment.
pub fn real_utility_with_env(env: &impl Env, utility: &str) -> Result<Utf8PathBuf> {
    which(env, utility)
}

/// Resolve a utility against the supplied `PATH`, before the sandbox replaces
/// it. Executability is part of the match because some installations place
/// non-executable shell fragments beside real tools.
pub(super) fn which(env: &impl Env, utility: &str) -> Result<Utf8PathBuf> {
    let path = env.raw("PATH").context("read PATH")?;
    let current_dir = std::env::current_dir().context("read current directory")?;
    for directory in std::env::split_paths(std::ffi::OsStr::new(&path)) {
        let absolute_directory = if directory.is_absolute() {
            directory
        } else {
            current_dir.join(directory)
        };
        let Ok(dir) = Utf8PathBuf::try_from(absolute_directory) else {
            continue;
        };
        let candidate = dir.join(utility);
        if fs::is_executable_file(candidate.as_std_path()) {
            return Ok(candidate);
        }
    }
    bail!("`{utility}` not found on PATH")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use mockable::MockEnv;

    #[test]
    fn relative_path_entries_resolve_to_absolute_executables() -> Result<()> {
        let current_dir = std::env::current_dir().context("read test current directory")?;
        let temp = tempfile::tempdir_in(&current_dir).context("create relative PATH fixture")?;
        crate::exec::write_exec(temp.path(), "tool").context("write fixture executable")?;
        let relative = temp
            .path()
            .strip_prefix(&current_dir)
            .context("fixture should be beneath the current directory")?
            .to_string_lossy()
            .into_owned();
        let mut env = MockEnv::new();
        env.expect_raw()
            .withf(|key| key == "PATH")
            .once()
            .return_once(move |_| Ok(relative));

        let resolved = which(&env, "tool")?;

        anyhow::ensure!(resolved.is_absolute(), "resolved utility must be absolute");
        anyhow::ensure!(
            resolved.as_std_path().is_file(),
            "resolved utility must exist"
        );
        Ok(())
    }
}
