//! Host-utility discovery for the hermetic dev-fast sandbox.

use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};
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
///
/// # Examples
///
/// ```rust,no_run
/// use test_support::dev_fast::real_utility;
///
/// let shell = real_utility("sh").expect("resolve the host shell");
/// assert!(shell.is_absolute());
/// ```
pub fn real_utility(utility: &str) -> Result<Utf8PathBuf> {
    let ambient_current_dir = std::env::current_dir().context("read current directory")?;
    let current_dir = Utf8PathBuf::try_from(ambient_current_dir).map_err(|error| {
        anyhow::anyhow!(
            "current directory is not UTF-8: {}",
            error.into_path_buf().display()
        )
    })?;
    real_utility_with_env(&DefaultEnv, &current_dir, utility)
}

/// Resolve a genuine utility using an injected environment.
///
/// This is the test seam for [`real_utility`]; it is not a general-purpose
/// executable discovery API.
///
/// # Errors
///
/// Returns an error if the utility cannot be resolved from the supplied environment.
///
/// # Examples
///
/// ```rust,no_run
/// use camino::Utf8Path;
/// use mockable::MockEnv;
/// use test_support::dev_fast::real_utility_with_env;
///
/// let mut env = MockEnv::new();
/// env.expect_raw()
///     .withf(|name| name == "PATH")
///     .return_once(|_| Ok(String::from("/empty/tool-directory")));
/// let error = real_utility_with_env(&env, Utf8Path::new("/workspace"), "tool")
///     .expect_err("the injected PATH has no tool");
/// assert!(error.to_string().contains("not found on PATH"));
/// ```
pub fn real_utility_with_env(
    env: &impl Env,
    current_dir: &Utf8Path,
    utility: &str,
) -> Result<Utf8PathBuf> {
    which(env, current_dir, utility)
}

/// Resolve a utility against the supplied `PATH`, before the sandbox replaces
/// it. Executability is part of the match because some installations place
/// non-executable shell fragments beside real tools.
///
/// Failures are traced with only a bounded `failure_kind`. The utility name,
/// `PATH`, and candidate paths are all omitted: they are high-cardinality and
/// disclose the host layout. The returned error still carries them, because a
/// developer reading a failed run needs to know which candidate broke.
pub(super) fn which(env: &impl Env, current_dir: &Utf8Path, utility: &str) -> Result<Utf8PathBuf> {
    let path = env.raw("PATH").context("read PATH")?;
    for directory in std::env::split_paths(std::ffi::OsStr::new(&path)) {
        let absolute_directory = if directory.is_absolute() {
            directory
        } else {
            current_dir.as_std_path().join(directory)
        };
        let Ok(dir) = Utf8PathBuf::try_from(absolute_directory) else {
            continue;
        };
        let candidate = dir.join(utility);
        match fs::executable_file(candidate.as_std_path()) {
            Ok(true) => return Ok(candidate),
            Ok(false) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::debug!(
                    failure_kind = "candidate_metadata",
                    "sandbox utility candidate inspection failed"
                );
                return Err(error)
                    .with_context(|| format!("inspect utility candidate `{candidate}`"));
            }
        }
    }
    tracing::debug!(outcome = "not_found", "sandbox utility lookup failed");
    bail!("`{utility}` not found on PATH")
}

#[cfg(all(test, unix))]
mod tests {
    //! Unit tests for hermetic utility resolution.

    use super::*;
    use crate::tracing_capture::with_test_subscriber;
    use mockable::MockEnv;
    use proptest::prelude::*;
    use proptest::test_runner::TestCaseError;
    use tracing_subscriber::filter::LevelFilter;

    /// Stands in for a secret embedded in a host path; it must never reach a
    /// log line, even though the returned error may name the candidate.
    const SENTINEL: &str = "s3cr3t-sentinel";

    /// Resolve `utility` against `path` and return the DEBUG events emitted.
    ///
    /// The lookup is required to fail: these are the diagnostic paths, and a
    /// fixture that accidentally resolved would assert nothing.
    fn captured_lookup_failure(
        temp: &tempfile::TempDir,
        path: String,
        utility: &str,
    ) -> Result<Vec<String>> {
        let mut env = MockEnv::new();
        env.expect_raw().return_once(move |_| Ok(path));
        let current_dir =
            Utf8Path::from_path(temp.path()).context("temporary path is not UTF-8")?;

        with_test_subscriber(LevelFilter::DEBUG, |captured| {
            let outcome = which(&env, current_dir, utility);
            anyhow::ensure!(
                outcome.is_err(),
                "the fixture lookup must fail so the diagnostic path is exercised"
            );
            Ok(captured.snapshot())
        })
    }

    /// Assert that `events` carries the expected bounded diagnostic and nothing
    /// more revealing than that.
    ///
    /// `privacy_context` names what the caller's fixture put at risk, so a
    /// leak reports which input escaped.
    fn assert_bounded_event(
        events: &[String],
        message: &str,
        bounded_field: &str,
        privacy_context: &str,
    ) -> Result<()> {
        anyhow::ensure!(
            events
                .iter()
                .any(|event| event.contains(message) && event.contains(bounded_field)),
            "expected an event containing {message:?} and {bounded_field:?} in {events:?}"
        );
        anyhow::ensure!(
            !events.iter().any(|event| event.contains(SENTINEL)),
            "{privacy_context}: {events:?}"
        );
        Ok(())
    }

    #[test]
    fn unresolved_utility_traces_only_a_bounded_outcome() -> Result<()> {
        let temp = tempfile::tempdir().context("create empty PATH fixture")?;
        let empty = temp.path().join(SENTINEL);
        fs::create_dir(&empty).context("create empty binary directory")?;
        let path = empty.to_string_lossy().into_owned();

        let events = captured_lookup_failure(&temp, path, SENTINEL)?;

        assert_bounded_event(
            &events,
            "sandbox utility lookup failed",
            "outcome=\"not_found\"",
            "the utility name and PATH must not be logged",
        )
    }

    #[test]
    fn candidate_metadata_failure_traces_only_a_bounded_failure_kind() -> Result<()> {
        let temp = tempfile::tempdir().context("create invalid PATH fixture")?;
        let not_a_directory = temp.path().join(SENTINEL);
        fs::write(&not_a_directory, "contents").context("write invalid PATH entry")?;
        let path = not_a_directory.to_string_lossy().into_owned();

        let events = captured_lookup_failure(&temp, path, "tool")?;

        assert_bounded_event(
            &events,
            "sandbox utility candidate inspection failed",
            "failure_kind=\"candidate_metadata\"",
            "the candidate path must not be logged",
        )
    }

    #[test]
    fn relative_path_entries_resolve_to_absolute_executables() -> Result<()> {
        let temp = tempfile::tempdir().context("create relative PATH fixture")?;
        let bin = temp.path().join("bin");
        fs::create_dir(&bin).context("create relative binary directory")?;
        crate::exec::write_exec(&bin, "tool").context("write fixture executable")?;
        let current_dir =
            Utf8Path::from_path(temp.path()).context("temporary path is not UTF-8")?;
        let mut env = MockEnv::new();
        env.expect_raw()
            .withf(|key| key == "PATH")
            .once()
            .return_once(|_| Ok(String::from("bin")));

        let resolved = which(&env, current_dir, "tool")?;

        anyhow::ensure!(resolved.is_absolute(), "resolved utility must be absolute");
        anyhow::ensure!(
            resolved.as_std_path().is_file(),
            "resolved utility must exist"
        );
        Ok(())
    }

    #[test]
    fn utility_lookup_returns_the_first_executable_match() -> Result<()> {
        let temp = tempfile::tempdir().context("create PATH fixture")?;
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        fs::create_dir(&first).context("create first binary directory")?;
        fs::create_dir(&second).context("create second binary directory")?;
        let first_tool = crate::exec::write_exec(&first, "tool")?;
        crate::exec::write_exec(&second, "tool")?;
        let path = std::env::join_paths([&first, &second])
            .context("join fixture PATH")?
            .to_string_lossy()
            .into_owned();
        let mut env = MockEnv::new();
        env.expect_raw().return_once(move |_| Ok(path));
        let current_dir =
            Utf8Path::from_path(temp.path()).context("temporary path is not UTF-8")?;

        let resolved = which(&env, current_dir, "tool")?;

        anyhow::ensure!(
            resolved.as_std_path() == first_tool,
            "first PATH match must win"
        );
        Ok(())
    }

    #[test]
    fn utility_lookup_propagates_non_missing_metadata_errors() -> Result<()> {
        let temp = tempfile::tempdir().context("create invalid PATH fixture")?;
        let not_a_directory = temp.path().join("file");
        fs::write(&not_a_directory, "contents").context("write invalid PATH entry")?;
        let path = not_a_directory.to_string_lossy().into_owned();
        let mut env = MockEnv::new();
        env.expect_raw().return_once(move |_| Ok(path));
        let current_dir =
            Utf8Path::from_path(temp.path()).context("temporary path is not UTF-8")?;

        let error = which(&env, current_dir, "tool")
            .expect_err("a non-directory PATH entry should preserve its metadata error");

        anyhow::ensure!(
            error.to_string().contains("inspect utility candidate"),
            "metadata error should retain candidate context, got {error:?}"
        );
        Ok(())
    }

    proptest! {
        #[test]
        fn relative_path_lookup_returns_the_first_generated_match(
            (directory_count, first_match) in (1usize..5)
                .prop_flat_map(|count| (Just(count), 0usize..count)),
        ) {
            let temp = tempfile::tempdir()
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            let mut directories = Vec::with_capacity(directory_count);
            for index in 0..directory_count {
                let relative = format!("bin{index}");
                let directory = temp.path().join(&relative);
                fs::create_dir(&directory)
                    .map_err(|error| TestCaseError::fail(error.to_string()))?;
                if index >= first_match {
                    crate::exec::write_exec(&directory, "tool")
                        .map_err(|error| TestCaseError::fail(error.to_string()))?;
                }
                directories.push(relative);
            }
            let path = std::env::join_paths(&directories)
                .map_err(|error| TestCaseError::fail(error.to_string()))?
                .to_string_lossy()
                .into_owned();
            let mut env = MockEnv::new();
            env.expect_raw().return_once(move |_| Ok(path));
            let current_dir = Utf8Path::from_path(temp.path())
                .ok_or_else(|| TestCaseError::fail("temporary path is not UTF-8"))?;

            let resolved = which(&env, current_dir, "tool")
                .map_err(|error| TestCaseError::fail(error.to_string()))?;

            prop_assert!(resolved.is_absolute());
            prop_assert_eq!(resolved, current_dir.join(format!("bin{first_match}/tool")));
        }
    }
}
