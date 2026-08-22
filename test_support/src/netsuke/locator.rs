//! Locating the built `netsuke` executable from a running test.
//!
//! Split from the parent `netsuke` module, which owns running the binary once
//! it is found. The two concerns are independent: everything here is pure path
//! reasoning over an injected environment, with the only filesystem contact
//! being the existence probe, whereas the parent module spawns processes.
//! Keeping them apart also keeps each within the module line cap.
//!
//! Reuse policy: private to `test_support::netsuke`. Tests reach the binary
//! through `run_netsuke_in` or `run_netsuke_in_with_env`, never through the
//! locator directly, so nothing here is exported from the crate.

use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};
use mockable::{DefaultEnv, Env};

/// Locate the built `netsuke` executable for integration-style tests.
///
/// Derive the primary path from the current test executable's directory, then
/// fall back to `CARGO_TARGET_DIR` when Cargo's `build.build-dir` splits
/// intermediate artefacts from final ones: test executables then run from the
/// build dir while the uplifted binary lands under the target dir.
pub(super) fn netsuke_executable() -> Result<Utf8PathBuf> {
    let raw_exe = std::env::current_exe().context("locate current test executable")?;
    let current_exe = Utf8PathBuf::from_path_buf(raw_exe)
        .map_err(|path| anyhow::anyhow!("test executable path {} is not UTF-8", path.display()))?;
    netsuke_executable_from(&DefaultEnv, &current_exe)
}

/// Reduces a test executable's directory to the profile directory above it.
///
/// Cargo has placed integration-test executables in two different places, and
/// the final binary is uplifted to the profile directory in both cases:
///
/// - `<profile>/deps/`, the long-standing layout;
/// - `<profile>/build/<package>/<hash>/out/`, used by the Cargo shipped with
///   the 1.99 nightlies, which has no `deps` directory at all.
///
/// Anything else is returned unchanged, so an unrecognized layout degrades to
/// looking beside the executable rather than failing here. The result also
/// supplies the profile name for the `CARGO_TARGET_DIR` fallbacks, so both
/// layouts derive the same name.
fn profile_dir(exe_dir: &Utf8Path) -> &Utf8Path {
    match exe_dir.file_name() {
        Some("deps") => exe_dir.parent().unwrap_or(exe_dir),
        Some("out") => {
            let build = exe_dir
                .parent()
                .and_then(Utf8Path::parent)
                .and_then(Utf8Path::parent);
            match build {
                Some(dir) if dir.file_name() == Some("build") => dir.parent().unwrap_or(exe_dir),
                _ => exe_dir,
            }
        }
        _ => exe_dir,
    }
}

/// Locate the `netsuke` binary from an injected environment and test path.
///
/// Candidates are checked in order:
/// 1. the profile directory above the test executable (see [`profile_dir`]);
/// 2. `CARGO_TARGET_DIR/<profile>/` for split `build.build-dir` layouts;
/// 3. `CARGO_TARGET_DIR/<triple>/<profile>/` for `--target` builds, where the
///    profile directory nests under the target triple.
///
/// Filesystem errors other than "not found" are surfaced rather than treated
/// as a missing candidate.
fn netsuke_executable_from(env: &impl Env, current_exe: &Utf8Path) -> Result<Utf8PathBuf> {
    let exe_dir = profile_dir(
        current_exe
            .parent()
            .context("test executable should have a parent directory")?,
    );

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
    // Cargo 1.99 nightlies drop `deps/` and run integration tests from a
    // build directory under the profile, uplifting the binary as before.
    #[case::build_out_primary(LocatorScenario::new(
        "target/debug/build/netsuke-build/abc123/out/test-exe",
        None,
        "target/debug",
        "build-out layout should resolve beside the profile"
    ))]
    #[case::build_out_profile_fallback(LocatorScenario::new(
        "build/debug/build/netsuke-build/abc123/out/test-exe",
        Some("target"),
        "target/debug",
        "build-out layout should reach the profile fallback"
    ))]
    // A directory merely named `out` is not the build layout, so the locator
    // must look beside the executable rather than four levels up.
    #[case::unrecognized_out_directory(LocatorScenario::new(
        "somewhere/out/test-exe",
        None,
        "somewhere/out",
        "an unrecognized `out` directory should not be stripped"
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
        for (slot, dir) in candidate_dirs.iter().enumerate() {
            if presence & (1 << slot) != 0 {
                touch(&root.join(dir).join(binary_name()))?;
            }
        }

        let located = netsuke_executable_from(&env_with_target_dir(Some(&target_dir)), &exe);
        match (0..candidate_dirs.len()).find(|slot| presence & (1 << slot) != 0) {
            Some(winner) => {
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
