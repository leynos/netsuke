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
    use rstest::{fixture, rstest};

    fn utf8_root(temp: &tempfile::TempDir) -> Result<Utf8PathBuf> {
        Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow::anyhow!("temp dir {} is not UTF-8", path.display()))
    }

    /// Temporary workspace for staging locator layouts.
    ///
    /// The fixture returns a `Result` so tests propagate setup failures with
    /// `?` instead of panicking; the `TempDir` half keeps the directory
    /// alive for the test body.
    type TempRoot = Result<(tempfile::TempDir, Utf8PathBuf)>;

    #[fixture]
    fn temp_root() -> TempRoot {
        let temp = tempfile::tempdir().context("create temp dir")?;
        let root = utf8_root(&temp)?;
        Ok((temp, root))
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

    /// One locator layout scenario, with paths relative to the temp root.
    struct LocatorScenario {
        exe_rel: &'static str,
        target_dir_rel: Option<&'static str>,
        binary_dir_rel: &'static str,
        message: &'static str,
    }

    impl LocatorScenario {
        const fn new(
            exe_rel: &'static str,
            target_dir_rel: Option<&'static str>,
            binary_dir_rel: &'static str,
            message: &'static str,
        ) -> Self {
            Self {
                exe_rel,
                target_dir_rel,
                binary_dir_rel,
                message,
            }
        }
    }

    /// Stage `scenario` under `root` and assert the resolved binary path.
    ///
    /// Touches the test executable and the expected binary, configures the
    /// mock environment with the scenario's target dir when supplied (all
    /// paths are relative to `root`), and asserts that the locator resolves
    /// the expected binary, retaining the scenario's message.
    fn assert_locates(root: &Utf8Path, scenario: &LocatorScenario) -> Result<()> {
        let exe = root.join(scenario.exe_rel);
        touch(&exe)?;
        let binary = root.join(scenario.binary_dir_rel).join(binary_name());
        touch(&binary)?;
        let target_dir = scenario.target_dir_rel.map(|rel| root.join(rel));

        let located = netsuke_executable_from(&env_with_target_dir(target_dir.as_deref()), &exe)?;
        ensure!(located == binary, "{}; got {located}", scenario.message);
        Ok(())
    }

    #[rstest]
    #[case::primary(LocatorScenario::new(
        "build/debug/deps/test-exe",
        None,
        "build/debug",
        "primary lookup should win"
    ))]
    #[case::profile_fallback(LocatorScenario::new(
        "build/debug/deps/test-exe",
        Some("target"),
        "target/debug",
        "profile fallback should resolve"
    ))]
    #[case::triple_fallback(LocatorScenario::new(
        "build/x86_64-unknown-linux-gnu/debug/deps/test-exe",
        Some("target"),
        "target/x86_64-unknown-linux-gnu/debug",
        "triple fallback should resolve"
    ))]
    fn resolves_each_candidate_layout(
        temp_root: TempRoot,
        #[case] scenario: LocatorScenario,
    ) -> Result<()> {
        let (_temp, root) = temp_root?;
        assert_locates(&root, &scenario)
    }

    #[rstest]
    fn prefers_the_primary_candidate_when_fallback_also_exists(temp_root: TempRoot) -> Result<()> {
        let (_temp, root) = temp_root?;
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

    /// Exhaustively covers every presence combination of the three
    /// candidates: the first present candidate in declaration order must win,
    /// and the empty combination must error. The eight cases enumerate the
    /// full presence mask, pinning the precedence invariant rather than
    /// sampling isolated layouts.
    #[rstest]
    #[case::none_present(0b000)]
    #[case::primary_only(0b001)]
    #[case::profile_only(0b010)]
    #[case::primary_and_profile(0b011)]
    #[case::triple_only(0b100)]
    #[case::primary_and_triple(0b101)]
    #[case::profile_and_triple(0b110)]
    #[case::all_present(0b111)]
    fn first_present_candidate_wins_for_every_presence_combination(
        temp_root: TempRoot,
        #[case] presence: u8,
    ) -> Result<()> {
        let candidate_dirs = ["build/debug", "target/debug", "target/build/debug"];
        let (_temp, root) = temp_root?;
        let exe = root.join("build/debug/deps/test-exe");
        touch(&exe)?;
        let target_dir = root.join("target");
        let present: Vec<usize> = (0..3).filter(|slot| presence & (1 << slot) != 0).collect();
        for &slot in &present {
            let dir = candidate_dirs.get(slot).context("slot in range")?;
            touch(&root.join(dir).join(binary_name()))?;
        }

        let located = netsuke_executable_from(&env_with_target_dir(Some(&target_dir)), &exe);
        match present.first() {
            Some(&winner) => {
                let dir = candidate_dirs.get(winner).context("winner in range")?;
                let expected = root.join(dir).join(binary_name());
                let resolved = located
                    .with_context(|| format!("combination {presence:#05b} should resolve"))?;
                ensure!(
                    resolved == expected,
                    "combination {presence:#05b} should pick candidate {winner}; got {resolved}"
                );
            }
            None => ensure!(
                located.is_err(),
                "combination {presence:#05b} has no binary and should error"
            ),
        }
        Ok(())
    }

    /// A candidate that cannot be inspected (traversal through a regular
    /// file) must surface the filesystem error rather than fall through to
    /// later candidates.
    #[rstest]
    fn surfaces_filesystem_errors_from_candidate_probes(temp_root: TempRoot) -> Result<()> {
        let (_temp, root) = temp_root?;
        // `build` is a regular file, so the first candidate `build/netsuke`
        // traverses through it and fails with an error other than NotFound.
        // The executable path itself never needs to exist for the probe.
        let blocker = root.join("build");
        touch(&blocker)?;
        let exe = blocker.join("test-exe");

        let error = netsuke_executable_from(&env_with_target_dir(None), &exe)
            .expect_err("probing through a regular file should fail");
        ensure!(
            error
                .to_string()
                .contains("inspect candidate netsuke binary"),
            "unexpected error: {error:?}"
        );
        Ok(())
    }

    #[rstest]
    fn reports_every_attempted_candidate_when_missing(temp_root: TempRoot) -> Result<()> {
        let (_temp, root) = temp_root?;
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
