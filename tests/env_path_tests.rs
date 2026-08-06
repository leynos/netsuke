//! Tests for composing isolated child-process `PATH` values.
//!
//! Composition is pure (`test_support::env::prepend_path_value`) and the
//! runner applies the result as data via `CommandEnv`, so nothing here
//! mutates the parent process: no test carries `#[serial]` and none needs
//! `EnvLock`.

use anyhow::{Context, Result, ensure};
use netsuke::runner::CommandEnv;
use proptest::prelude::*;
use rstest::{fixture, rstest};
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
/// environment; on Unix the program itself is still resolved by the parent's
/// `PATH`, so injecting a directory does not make a bare `ninja` name resolve
/// to a fake. That is why callers pass an explicit program path. What the
/// injected `PATH` governs is the environment Ninja itself runs build commands
/// under, which is the thing tests need to control.
#[cfg(unix)]
#[rstest]
#[expect(
    clippy::disallowed_methods,
    reason = "the parent's real PATH is both the base for the composed value and the subject of the closing assertion that spawning left it unchanged, so it must be observed directly"
)]
fn composed_path_reaches_the_spawned_process(
    probe_fixture: Result<(tempfile::TempDir, PathBuf, PathBuf)>,
) -> Result<()> {
    use netsuke::cli::Cli;
    use netsuke::runner::{BuildTargets, NinjaBuildRequest, run_ninja_with};
    use std::path::Path;
    let (dir, probe, build_file) = probe_fixture?;
    let parent_before = std::env::var_os("PATH");
    let composed = prepend_path_value(parent_before.as_deref(), Path::new("/injected/marker"))
        .context("compose PATH")?;
    let cli = Cli::default();
    let targets = BuildTargets::default();

    run_ninja_with(&NinjaBuildRequest {
        program: probe.as_path(),
        cli: &cli,
        build_file: build_file.as_path(),
        targets: &targets,
        env: &CommandEnv::inherit().with_path(&composed),
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
    use netsuke::cli::Cli;
    use netsuke::runner::{BuildTargets, NinjaBuildRequest, run_ninja_with};

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
    let cli = Cli::default();
    let targets = BuildTargets::default();

    // The override touches only an unrelated marker; PATH is not configured.
    run_ninja_with(&NinjaBuildRequest {
        program: probe.as_path(),
        cli: &cli,
        build_file: build_file.as_path(),
        targets: &targets,
        env: &CommandEnv::inherit().with_var("NETSUKE_PROBE_MARKER", "sentinel"),
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
    use netsuke::cli::Cli;
    use netsuke::runner::{NinjaToolRequest, run_ninja_tool_with};

    let (dir, probe, build_file) = probe_fixture?;
    let cli = Cli::default();

    run_ninja_tool_with(&NinjaToolRequest {
        program: probe.as_path(),
        cli: &cli,
        build_file: build_file.as_path(),
        tool: "clean",
        env: &CommandEnv::inherit().with_var("NETSUKE_PROBE_MARKER", "sentinel"),
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

mod properties {
    //! Property coverage for `CommandEnv` and `PATH` composition.
    //!
    //! The fixed cases above name specific behaviours; these state the
    //! invariants they are instances of, over inputs nobody would write down:
    //! arbitrary entry lists including empty entries, and arbitrary override
    //! sequences with repeated keys.

    use netsuke::runner::CommandEnv;
    use proptest::collection::vec;
    use proptest::prelude::*;
    use std::collections::HashMap;
    use std::ffi::{OsStr, OsString};
    use std::path::{Path, PathBuf};
    use test_support::env::prepend_path_value;

    /// One `PATH` entry: separator-free on every platform, possibly empty.
    ///
    /// Empty entries are generated deliberately — they are meaningful on Unix
    /// (the current directory) and must survive composition when they sit
    /// inside a non-empty value.
    fn entry() -> impl Strategy<Value = String> {
        prop_oneof![
            1 => Just(String::new()),
            4 => "[A-Za-z0-9_./-]{1,8}",
        ]
    }

    proptest! {
        /// Composition prepends `dir` and preserves the existing entries and
        /// their order exactly; an absent value yields only `dir`, and — by
        /// `prepend_path_value`'s contract — so does a wholly empty one.
        ///
        /// The expectation splits the *input* value rather than echoing the
        /// generated list, because `join_paths` cannot distinguish an empty
        /// list from one empty entry — that ambiguity belongs to the `PATH`
        /// representation, and the helper's contract is over the value it
        /// receives. A dropped or reordered entry still cannot agree with
        /// itself, since input and output are split independently.
        #[test]
        fn composition_prepends_and_preserves_order(
            existing in prop::option::of(vec(entry(), 0..6)),
            dir in "[A-Za-z0-9_./-]{1,8}",
        ) {
            let joined = existing
                .as_ref()
                .map(|parts| std::env::join_paths(parts).expect("separator-free entries join"));
            let composed = prepend_path_value(joined.as_deref(), Path::new(&dir))
                .expect("separator-free inputs compose");

            let mut expected = vec![PathBuf::from(&dir)];
            if let Some(value) = joined.as_deref().filter(|value| !value.is_empty()) {
                expected.extend(std::env::split_paths(value));
            }
            let found: Vec<PathBuf> = std::env::split_paths(&composed).collect();
            prop_assert_eq!(found, expected);
        }

        /// `get` answers per the last override for each key, and `is_empty`
        /// holds exactly when no override was ever set.
        ///
        /// The model is a plain last-write-wins map, independent of the
        /// implementation's in-place-update vector, so a bookkeeping slip
        /// between lookup and storage fails here.
        #[test]
        fn overrides_resolve_to_their_last_declaration(
            ops in vec(("[AB]", "[a-z]{0,4}"), 0..8)
        ) {
            let mut model: HashMap<String, String> = HashMap::new();
            let mut env = CommandEnv::inherit();
            for (key, value) in &ops {
                model.insert(key.clone(), value.clone());
                env = env.with_var(key, value);
            }
            prop_assert_eq!(env.is_empty(), model.is_empty());
            for key in ["A", "B", "C"] {
                let expected = model.get(key).map(|value| OsString::from(value.clone()));
                prop_assert_eq!(
                    env.get(key),
                    expected.as_deref().map(OsStr::new),
                    "key {}", key
                );
            }
        }
    }
}
