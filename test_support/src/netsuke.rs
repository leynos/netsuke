//! Helpers for invoking the built `netsuke` binary in tests.
//!
//! These utilities use `assert_cmd` to locate the current workspace's
//! `netsuke` executable and run it in a controlled working directory,
//! capturing stdout/stderr for assertions.

use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};
use mockable::{DefaultEnv, Env};
use std::path::Path;

/// Locate the built `netsuke` executable for integration-style tests.
///
/// Derive the primary path from the current test executable's directory, then
/// fall back to `CARGO_TARGET_DIR` when Cargo's `build.build-dir` splits
/// intermediate artefacts from final ones: test executables then run from the
/// build dir while the uplifted binary lands under the target dir.
fn netsuke_executable() -> Result<Utf8PathBuf> {
    let raw_exe = std::env::current_exe().context("locate current test executable")?;
    let current_exe = Utf8PathBuf::from_path_buf(raw_exe)
        .map_err(|path| anyhow::anyhow!("test executable path {} is not UTF-8", path.display()))?;
    netsuke_executable_from(&DefaultEnv, &current_exe)
}

/// Locate the `netsuke` binary from an injected environment and test path.
///
/// Candidates are checked in order:
/// 1. beside the test executable (its directory, minus a trailing `deps`);
/// 2. `CARGO_TARGET_DIR/<profile>/` for split `build.build-dir` layouts;
/// 3. `CARGO_TARGET_DIR/<triple>/<profile>/` for `--target` builds, where the
///    profile directory nests under the target triple.
///
/// Filesystem errors other than "not found" are surfaced rather than treated
/// as a missing candidate.
fn netsuke_executable_from(env: &impl Env, current_exe: &Utf8Path) -> Result<Utf8PathBuf> {
    let mut exe_dir = current_exe
        .parent()
        .context("test executable should have a parent directory")?;
    if exe_dir.file_name() == Some("deps") {
        exe_dir = exe_dir
            .parent()
            .context("deps directory should have a parent")?;
    }

    let binary_name = format!("netsuke{}", std::env::consts::EXE_SUFFIX);
    let candidates = candidate_paths(env, exe_dir, &binary_name);
    for candidate in &candidates {
        let is_file = crate::fs::try_is_file(candidate)
            .with_context(|| format!("inspect candidate netsuke binary at {candidate}"))?;
        if is_file {
            return Ok(candidate.clone());
        }
    }
    let attempted = candidates
        .iter()
        .map(|candidate| candidate.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    bail!("locate netsuke binary; tried: {attempted}");
}

/// Build the ordered candidate paths for the `netsuke` binary.
fn candidate_paths(env: &impl Env, exe_dir: &Utf8Path, binary_name: &str) -> Vec<Utf8PathBuf> {
    let mut candidates = vec![exe_dir.join(binary_name)];
    let (Some(target_dir), Some(profile)) = (env.string("CARGO_TARGET_DIR"), exe_dir.file_name())
    else {
        return candidates;
    };
    let target_root = Utf8PathBuf::from(target_dir);
    candidates.push(target_root.join(profile).join(binary_name));
    // `--target` builds nest the profile directory under the target triple in
    // both the build dir and the target dir, so reinsert the component above
    // the profile when one exists. For no-`--target` builds that component is
    // the build-dir root, which never exists under the target dir, so the
    // extra candidate is harmless.
    if let Some(triple) = exe_dir.parent().and_then(Utf8Path::file_name) {
        candidates.push(target_root.join(triple).join(profile).join(binary_name));
    }
    candidates
}

/// Captured output from a `netsuke` invocation.
#[derive(Debug)]
pub struct NetsukeRun {
    /// Captured stdout (lossy UTF-8).
    pub stdout: String,
    /// Captured stderr (lossy UTF-8).
    pub stderr: String,
    /// Whether the command exited successfully.
    pub success: bool,
}

/// Run `netsuke` in `current_dir` with the supplied args.
///
/// The function clears `PATH` so tests don't accidentally execute a host
/// dependency. Other process environment variables are inherited, except for
/// configuration selectors that this helper removes explicitly.
///
/// # Errors
///
/// Returns an error when `netsuke` cannot be located or the process cannot be
/// spawned.
pub fn run_netsuke_in(current_dir: &Path, args: &[&str]) -> Result<NetsukeRun> {
    let isolated_config_home = current_dir.join(".config");
    let executable = netsuke_executable()?;
    let mut cmd = assert_cmd::Command::new(executable);
    let output = cmd
        .current_dir(current_dir)
        .env("PATH", "")
        .env_remove("NETSUKE_CONFIG_PATH")
        .env_remove("NETSUKE_OUTPUT_FORMAT")
        .env("HOME", current_dir)
        .env("XDG_CONFIG_HOME", &isolated_config_home)
        .args(args)
        .output()
        .context("run netsuke command")?;
    Ok(NetsukeRun {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Run `netsuke` in `current_dir` with an isolated environment.
///
/// Unlike [`run_netsuke_in`], this variant uses `env_clear()` so the child
/// inherits no process environment variables. The child receives only an
/// isolated `PATH`, `HOME`, `XDG_CONFIG_HOME`, and the variables supplied in
/// `extra_env`. This prevents process-level environment races when tests run
/// in parallel.
///
/// # Errors
///
/// Returns an error when `netsuke` cannot be located or the process cannot be
/// spawned.
pub fn run_netsuke_in_with_env(
    current_dir: &Path,
    args: &[&str],
    extra_env: &[(&str, &str)],
) -> Result<NetsukeRun> {
    let executable = netsuke_executable()?;
    let mut cmd = assert_cmd::Command::new(executable);
    let isolated_config_home = current_dir.join(".config");
    let isolated_path = tempfile::tempdir().context("create isolated executable directory")?;
    cmd.current_dir(current_dir)
        .env_clear()
        .env("PATH", isolated_path.path())
        .env("HOME", current_dir)
        .env("XDG_CONFIG_HOME", isolated_config_home);
    for &(key, value) in extra_env {
        cmd.env(key, value);
    }
    let output = cmd.args(args).output().context("run netsuke command")?;
    Ok(NetsukeRun {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

#[cfg(test)]
mod tests {
    //! Unit tests for the netsuke binary locator.

    use super::netsuke_executable_from;
    use anyhow::{Context, Result, ensure};
    use camino::{Utf8Path, Utf8PathBuf};
    use mockable::MockEnv;

    fn utf8_root(temp: &tempfile::TempDir) -> Result<Utf8PathBuf> {
        Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow::anyhow!("temp dir {} is not UTF-8", path.display()))
    }

    fn binary_name() -> String {
        format!("netsuke{}", std::env::consts::EXE_SUFFIX)
    }

    fn touch(path: &Utf8Path) -> Result<()> {
        let parent = path.parent().context("path should have a parent")?;
        crate::fs::create_dir_all(parent).with_context(|| format!("create {parent}"))?;
        crate::fs::write(path, b"stub").with_context(|| format!("write {path}"))?;
        Ok(())
    }

    fn env_with_target_dir(target_dir: Option<&Utf8Path>) -> MockEnv {
        let mut env = MockEnv::new();
        let value = target_dir.map(Utf8Path::to_string);
        env.expect_string()
            .withf(|key| key == "CARGO_TARGET_DIR")
            .return_const(value);
        env
    }

    /// Stage a locator scenario and assert the resolved binary path.
    ///
    /// Creates the temporary root, touches the test executable at `exe_rel`
    /// and the expected binary at `binary_rel`, configures the mock
    /// environment with `target_dir_rel` when supplied (all three paths are
    /// relative to the root), and asserts that the locator resolves the
    /// expected binary, retaining `message` in the diagnostic.
    fn assert_locates(
        exe_rel: &str,
        target_dir_rel: Option<&str>,
        binary_rel: &str,
        message: &str,
    ) -> Result<()> {
        let temp = tempfile::tempdir().context("create temp dir")?;
        let root = utf8_root(&temp)?;
        let exe = root.join(exe_rel);
        touch(&exe)?;
        let binary = root.join(binary_rel);
        touch(&binary)?;
        let target_dir = target_dir_rel.map(|rel| root.join(rel));

        let located = netsuke_executable_from(&env_with_target_dir(target_dir.as_deref()), &exe)?;
        ensure!(located == binary, "{message}; got {located}");
        Ok(())
    }

    #[test]
    fn locates_binary_beside_the_test_executable() -> Result<()> {
        assert_locates(
            "build/debug/deps/test-exe",
            None,
            &format!("build/debug/{}", binary_name()),
            "primary lookup should win",
        )
    }

    #[test]
    fn falls_back_to_cargo_target_dir_profile() -> Result<()> {
        assert_locates(
            "build/debug/deps/test-exe",
            Some("target"),
            &format!("target/debug/{}", binary_name()),
            "profile fallback should resolve",
        )
    }

    #[test]
    fn falls_back_to_target_triple_directory() -> Result<()> {
        assert_locates(
            "build/x86_64-unknown-linux-gnu/debug/deps/test-exe",
            Some("target"),
            &format!("target/x86_64-unknown-linux-gnu/debug/{}", binary_name()),
            "triple fallback should resolve",
        )
    }

    #[test]
    fn prefers_the_primary_candidate_when_fallback_also_exists() -> Result<()> {
        let temp = tempfile::tempdir().context("create temp dir")?;
        let root = utf8_root(&temp)?;
        let exe = root.join("build/debug/deps/test-exe");
        touch(&exe)?;
        let primary = root.join("build/debug").join(binary_name());
        touch(&primary)?;
        let target_dir = root.join("target");
        let fallback = target_dir.join("debug").join(binary_name());
        touch(&fallback)?;

        let located = netsuke_executable_from(&env_with_target_dir(Some(&target_dir)), &exe)?;
        ensure!(
            located == primary,
            "the primary candidate should win over the fallback; got {located}"
        );
        Ok(())
    }

    #[test]
    fn reports_every_attempted_candidate_when_missing() -> Result<()> {
        let temp = tempfile::tempdir().context("create temp dir")?;
        let root = utf8_root(&temp)?;
        let exe = root.join("build/debug/deps/test-exe");
        touch(&exe)?;
        let target_dir = root.join("target");

        let error = netsuke_executable_from(&env_with_target_dir(Some(&target_dir)), &exe)
            .expect_err("no candidate exists");
        let message = error.to_string();
        for expected in [
            root.join("build/debug").join(binary_name()),
            target_dir.join("debug").join(binary_name()),
            target_dir.join("build/debug").join(binary_name()),
        ] {
            ensure!(
                message.contains(expected.as_str()),
                "error should list attempted candidate {expected}; got: {message}"
            );
        }
        Ok(())
    }
}
