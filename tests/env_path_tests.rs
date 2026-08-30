//! Tests for composing isolated child-process `PATH` values.
//!
//! Composition is pure (`test_support::env::prepend_path_value`) and the
//! runner applies the result as data via `CommandEnv`, so nothing here
//! mutates the parent process: no test carries `#[serial]` and none needs
//! process-global environment or working-directory coordination.
//!
//! These are the named cases; the invariants they instantiate live in
//! `env_path_property_tests.rs`, which Cargo builds as its own target.

use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use netsuke::runner::CommandEnv;
use proptest::prelude::*;
#[cfg(unix)]
use rstest::fixture;
use rstest::rstest;
use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};
use test_support::env::prepend_path_value;

#[rstest]
fn prepend_dir_to_path_preserves_existing_entries() -> Result<()> {
    let original = std::env::join_paths(["one", "two"])?;
    let dir = tempfile::tempdir().context("create temp dir")?;
    let composed = prepend_path_value(Some(&original), dir.path())?;
    let mut split_paths = std::env::split_paths(&composed);
    let first = split_paths
        .next()
        .context("PATH should contain at least one entry after prepend")?;
    ensure!(
        first == dir.path(),
        "expected {} to be first PATH entry, got {}",
        dir.path().display(),
        first.display()
    );
    let remaining = split_paths.collect::<Vec<_>>();
    ensure!(
        remaining == ["one", "two"].map(std::path::PathBuf::from),
        "existing PATH entries should retain their order"
    );
    Ok(())
}

/// Empty and absent starting values both yield only the new directory.
///
/// One parameterized case rather than two twins: the contract under test is
/// identical — a valueless start contributes nothing — and only the spelling
/// of "valueless" differs.
#[rstest]
#[case::empty(Some(OsStr::new("")))]
#[case::missing(None)]
fn prepend_dir_to_path_collapses_valueless_starts(#[case] existing: Option<&OsStr>) -> Result<()> {
    let dir = tempfile::tempdir().context("create temp dir")?;
    let composed = prepend_path_value(existing, dir.path())?;
    let paths: Vec<_> = std::env::split_paths(&composed).collect();
    ensure!(
        paths == vec![dir.path().to_path_buf()],
        "expected PATH to contain only {}; got {paths:?}",
        dir.path().display()
    );
    Ok(())
}

/// A directory that `PATH` cannot represent is rejected.
///
/// The unrepresentable character differs by platform: Unix rejects `:`
/// because entries cannot be quoted, whereas Windows can represent `;` by
/// quoting the entry and instead rejects `"`, the quoting character itself.
/// `join_paths` reports both, because the joined string could not be split
/// back into the same entries. The constant is selected by `cfg!` so the
/// case exercises each host's real rejection.
#[rstest]
fn an_unrepresentable_entry_is_rejected() -> Result<()> {
    const BAD_DIR: &str = if cfg!(windows) { "bad\"dir" } else { "bad:dir" };
    ensure!(
        prepend_path_value(Some(OsStr::new("/usr/bin")), std::path::Path::new(BAD_DIR)).is_err(),
        "an unrepresentable entry should be an error"
    );
    Ok(())
}

/// Composing must neither read nor write the process `PATH`.
#[rstest]
#[expect(
    clippy::disallowed_methods,
    reason = "the assertion under test is precisely that the real process PATH is untouched, so it must be observed directly rather than through an injected seam"
)]
fn composition_leaves_the_parent_path_unchanged() -> Result<()> {
    let before = std::env::var_os("PATH");
    let composed = prepend_path_value(Some(OsStr::new("/seeded")), std::path::Path::new("/fake"))
        .context("compose PATH")?;
    let env = CommandEnv::inherit().with_path(&composed);
    ensure!(
        env.get("PATH") == Some(composed.as_os_str()),
        "the composed value should reach the command environment"
    );
    ensure!(
        std::env::var_os("PATH") == before,
        "composing a PATH must not alter the parent process"
    );
    Ok(())
}

#[rstest]
fn later_overrides_replace_earlier_ones_for_the_same_key() -> Result<()> {
    let env = CommandEnv::inherit()
        .with_path("/first")
        .with_path("/second");
    ensure!(
        env.get("PATH") == Some(OsStr::new("/second")),
        "the later override should win, got {:?}",
        env.get("PATH")
    );
    Ok(())
}

#[rstest]
fn inherit_carries_no_overrides() {
    assert!(CommandEnv::inherit().is_empty());
}

/// A composed value round-trips through `CommandEnv` unaltered.
#[rstest]
fn composed_values_reach_the_command_environment_verbatim() -> Result<()> {
    let dir = tempfile::tempdir().context("create temp dir")?;
    let composed =
        prepend_path_value(Some(OsStr::new("/usr/bin")), dir.path()).context("compose PATH")?;
    let env = CommandEnv::inherit().with_path(&composed);
    ensure!(
        env.get("PATH").map(OsString::from) == Some(composed),
        "the command environment should carry the composed value verbatim"
    );
    Ok(())
}

/// Fixture for the child-probe tests: a fake Ninja recording one value.
///
/// Shared by every subprocess case so each states only what it configures
/// (via `#[with("...")]`) and what the child must have seen.
#[cfg(unix)]
#[fixture]
fn probe_fixture(
    #[default("printf '%s' \"$PATH\" > \"$0.observed\"")] script_line: &str,
) -> Result<(tempfile::TempDir, PathBuf, PathBuf)> {
    use test_support::fs as test_fs;

    let dir = tempfile::tempdir().context("create temp dir")?;
    let probe = dir.path().join("ninja");
    test_fs::write(&probe, format!("#!/bin/sh\n{script_line}\nexit 0\n")).context("write probe")?;
    test_fs::set_mode(&probe, 0o755).context("chmod probe")?;
    let build_file = dir.path().join("build.ninja");
    test_fs::write(&build_file, "rule noop\n  command = true\n").context("write build file")?;
    Ok((dir, probe, build_file))
}

/// The value the probe recorded, as raw bytes.
#[cfg(unix)]
fn observed_value(dir: &tempfile::TempDir) -> Result<std::ffi::OsString> {
    use std::os::unix::ffi::OsStringExt;
    let bytes = test_support::fs::read(dir.path().join("ninja.observed"))
        .context("probe should have recorded the value it saw")?;
    Ok(std::ffi::OsString::from_vec(bytes))
}

/// A composed `PATH` reaches the spawned process, and the parent keeps its own.
///
/// This is the end-to-end proof that injection works: the stand-in Ninja prints
/// the `PATH` it was given, so the assertion is on what the *child* actually
/// saw rather than on what was configured for it.
///
/// Note what this does and does not show. `Command::env` sets the child's
/// environment, and a bare relative program name such as `ninja` is looked
/// up in the *child's* `PATH` — injected directories included — so an
/// injected `PATH` could select the executable itself. That is why callers
/// needing executable-selection isolation pass an absolute or otherwise
/// resolved program path, as this test does. The injected `PATH` then only
/// governs the environment Ninja itself runs build commands under.
#[cfg(unix)]
#[rstest]
#[expect(
    clippy::disallowed_methods,
    reason = "the parent's real PATH is both the base for the composed value and the subject of the closing assertion that spawning left it unchanged, so it must be observed directly"
)]
fn composed_path_reaches_the_spawned_process(
    probe_fixture: Result<(tempfile::TempDir, PathBuf, PathBuf)>,
) -> Result<()> {
    use camino::Utf8PathBuf;
    use netsuke::runner::{
        BuildTargets, NinjaBuildRequest, NinjaProcessOptions, StderrMode, run_ninja_with,
    };
    use std::path::Path;
    let (dir, probe, build_file) = probe_fixture?;
    let utf8_probe = Utf8PathBuf::from_path_buf(probe)
        .map_err(|path| anyhow::anyhow!("probe path is not UTF-8: {}", path.display()))?;
    let utf8_build_file = Utf8PathBuf::from_path_buf(build_file)
        .map_err(|path| anyhow::anyhow!("build file path is not UTF-8: {}", path.display()))?;
    let parent_before = std::env::var_os("PATH");
    let composed = prepend_path_value(parent_before.as_deref(), Path::new("/injected/marker"))
        .context("compose PATH")?;
    let options = NinjaProcessOptions::default();
    let targets = BuildTargets::default();

    run_ninja_with(&NinjaBuildRequest {
        program: &utf8_probe,
        options: &options,
        build_file: &utf8_build_file,
        targets: &targets,
        env: &CommandEnv::inherit().with_path(&composed),
        stderr_mode: StderrMode::Forward,
    })
    .context("run the probe")?;

    // Compared as raw bytes: a valid Unix PATH may contain non-UTF-8, and a
    // lossy string round trip would fail before propagation was checked.
    let observed = observed_value(&dir)?;
    ensure!(
        observed == composed,
        "child PATH should equal the composed value;\n  saw:      {observed:?}\n  expected: {composed:?}"
    );
    ensure!(
        std::env::var_os("PATH") == parent_before,
        "spawning with an injected PATH must leave the parent unchanged"
    );
    Ok(())
}

/// Un-overridden parent variables are inherited by the spawned process.
///
/// The override mechanism is deliberately additive: `apply` sets only the
/// configured variables and never clears the child's environment. Without
/// this case an `env_clear`-based implementation would pass every other
/// test here, since they only ever assert on variables that were set.
#[cfg(unix)]
#[rstest]
fn unoverridden_parent_variables_are_inherited(
    #[from(probe_fixture)] baseline: Result<(tempfile::TempDir, PathBuf, PathBuf)>,
    probe_fixture: Result<(tempfile::TempDir, PathBuf, PathBuf)>,
) -> Result<()> {
    use netsuke::runner::{
        BuildTargets, NinjaBuildRequest, NinjaProcessOptions, StderrMode, run_ninja_with,
    };

    // Baseline: the probe spawned directly, outside `CommandEnv`, records
    // the PATH a plainly inherited child sees. Comparing child against child
    // keeps the parent's environment unread while still failing an
    // `env_clear`-based implementation, whose run below would record an
    // empty PATH where this one records the real value.
    let (baseline_dir, baseline_probe, _) = baseline?;
    let status = std::process::Command::new(&baseline_probe)
        .status()
        .context("run the baseline probe")?;
    ensure!(status.success(), "the baseline probe should exit cleanly");
    let inherited = observed_value(&baseline_dir)?;

    let (dir, probe, build_file) = probe_fixture?;
    let utf8_probe = Utf8PathBuf::from_path_buf(probe)
        .map_err(|path| anyhow::anyhow!("probe path is not UTF-8: {}", path.display()))?;
    let utf8_build_file = Utf8PathBuf::from_path_buf(build_file)
        .map_err(|path| anyhow::anyhow!("build file path is not UTF-8: {}", path.display()))?;
    let options = NinjaProcessOptions::default();
    let targets = BuildTargets::default();

    // The override touches only an unrelated marker; PATH is not configured.
    run_ninja_with(&NinjaBuildRequest {
        program: &utf8_probe,
        options: &options,
        build_file: &utf8_build_file,
        targets: &targets,
        env: &CommandEnv::inherit().with_var("NETSUKE_PROBE_MARKER", "sentinel"),
        stderr_mode: StderrMode::Forward,
    })
    .context("run the probe")?;

    let observed = observed_value(&dir)?;
    ensure!(
        observed == inherited,
        "an un-overridden PATH should be inherited from the parent;\n  saw:      {observed:?}\n  expected: {inherited:?}"
    );
    Ok(())
}

/// A general (non-`PATH`) override reaches the child through the tool boundary.
///
/// `PATH` gets its own composition helper, so it is the variable every other
/// test exercises; this one pins the plain `with_var` route and the
/// `NinjaToolRequest` boundary, proving both halves of the seam the review
/// asked about: an arbitrary variable, and the tool-request path carrying it.
#[cfg(unix)]
#[rstest]
fn general_overrides_reach_the_spawned_tool_process(
    #[with("printf '%s' \"$NETSUKE_PROBE_MARKER\" > \"$0.observed\"")] probe_fixture: Result<(
        tempfile::TempDir,
        PathBuf,
        PathBuf,
    )>,
) -> Result<()> {
    use netsuke::runner::{NinjaProcessOptions, NinjaToolRequest, StderrMode, run_ninja_tool_with};

    let (dir, probe, build_file) = probe_fixture?;
    let utf8_probe = Utf8PathBuf::from_path_buf(probe)
        .map_err(|path| anyhow::anyhow!("probe path is not UTF-8: {}", path.display()))?;
    let utf8_build_file = Utf8PathBuf::from_path_buf(build_file)
        .map_err(|path| anyhow::anyhow!("build file path is not UTF-8: {}", path.display()))?;
    let options = NinjaProcessOptions::default();

    run_ninja_tool_with(&NinjaToolRequest {
        program: &utf8_probe,
        options: &options,
        build_file: &utf8_build_file,
        tool: "clean",
        env: &CommandEnv::inherit().with_var("NETSUKE_PROBE_MARKER", "sentinel"),
        stderr_mode: StderrMode::Forward,
    })
    .context("run the probe")?;

    let observed = observed_value(&dir)?;
    ensure!(
        observed.as_os_str() == std::ffi::OsStr::new("sentinel"),
        "the child should see the injected variable, saw {observed:?}"
    );
    Ok(())
}

proptest! {
    #[test]
    fn prepend_dir_to_path_preserves_every_generated_entry(
        entries in prop::collection::vec("[A-Za-z0-9._-]{1,8}", 0..16),
    ) {
        let original = std::env::join_paths(&entries)
            .expect("generated PATH entries should be joinable");
        let dir = tempfile::tempdir().expect("create property-test temp dir");

        let composed = prepend_path_value(Some(&original), dir.path())
            .expect("prepend generated PATH entries");
        let actual = std::env::split_paths(&composed).collect::<Vec<_>>();
        let expected = std::iter::once(dir.path().to_path_buf())
            .chain(entries.into_iter().map(PathBuf::from))
            .collect::<Vec<_>>();

        prop_assert_eq!(actual, expected);
    }
}
