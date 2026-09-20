//! Compile-time tests for `StubEnv`'s builder-only construction.
//!
//! `StubEnv` deliberately does not implement `Default`: on a strict stub it
//! would mean "deny every read", so `StubEnv::default()` would compile and
//! then panic at run time for the common "no locale set" case. These tests
//! keep that a compile-time contract rather than a doc-comment promise.
//!
//! Trybuild drove these during the Polonius migration and could not: it
//! removes ambient `RUSTFLAGS` and overrides workspace `build.rustflags`
//! outright (`env_remove("RUSTFLAGS")` plus `--config=build.rustflags=…` in
//! its cargo invocations), so while Polonius was flag-gated it rebuilt the
//! `test_support` dependency without the analysis and rejected the crate's
//! `POLONIUS(...)` sites (see docs/polonius.md). The pinned nightly now
//! enables Polonius by default, so that hazard is gone, but the direct-compile
//! harness is kept: it needs no scratch project and no toolchain-sensitive
//! `.stderr` snapshot. The `test_support` rlib is built by Cargo, and the
//! fixtures are compiled directly with the workspace `rustc` against it.

#[path = "support/test_support_rlib.rs"]
mod test_support_rlib;

use rstest::{fixture, rstest};
use std::{io, path::Path};

/// One `test_support` build shared by both tests.
///
/// Built once: the two tests run in parallel, so independent builds would
/// contend on Cargo's target-directory lock and repeat completed work.
#[fixture]
#[once]
fn test_support_build() -> test_support_rlib::TestSupportRlib {
    #[expect(
        clippy::expect_used,
        reason = "a once fixture cannot return Result; a build failure must abort the suite here"
    )]
    let rlib = test_support_rlib::TestSupportRlib::build().expect("test_support should build");
    rlib
}

#[rstest]
fn stub_env_default_does_not_compile(
    test_support_build: &test_support_rlib::TestSupportRlib,
) -> io::Result<()> {
    let output = test_support_build.compile("tests/ui/stub_env_default_compile_fail.rs")?;

    if output.status.success() {
        return Err(io::Error::other("StubEnv::default() should not compile"));
    }
    let stderr = test_support_rlib::stderr(&output);
    if !stderr.contains("E0599") || !stderr.contains("`default`") {
        return Err(io::Error::other(format!(
            concat!(
                "the rejection should be the missing `default` item, ",
                "not a harness fault:\n{}",
            ),
            stderr
        )));
    }
    Ok(())
}

/// The builder constructors compile under the same harness.
///
/// This is the control for the compile-fail case: it fails if the `--extern`
/// or `-L dependency` wiring breaks, which would otherwise make the rejection
/// above pass for the wrong reason.
#[rstest]
fn stub_env_builders_compile_under_the_same_harness(
    test_support_build: &test_support_rlib::TestSupportRlib,
) -> io::Result<()> {
    let output = test_support_build.compile("tests/ui/stub_env_strict_compile_pass.rs")?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "the control fixture should compile; the harness wiring is broken:\n{}",
            test_support_rlib::stderr(&output),
        )));
    }
    Ok(())
}

/// Verify Cargo JSON preserves split-build dependency directories.
///
/// This parser regression uses a recorded split layout because its contract is
/// the `compiler-artifact` message interpretation, not Cargo's own build. A
/// live private build recompiles the workspace and races the shared fixture's
/// uplifted rlibs (`E0460`) if it uses the ambient target directory.
#[rstest]
fn harness_compiles_under_a_split_build_dir() -> io::Result<()> {
    const SPLIT_BUILD_DIR: &str = "/recorded/split-build";
    const UPLIFTED_TARGET_DIR: &str = "/recorded/uplifted-target";

    let messages = include_str!("ui/split_build_dir_cargo_messages.jsonl");
    let dependency_dirs = messages
        .lines()
        .flat_map(test_support_rlib::cargo_artifacts::dependency_dirs_in_message)
        .collect::<Vec<_>>();
    if !dependency_dirs
        .iter()
        .any(|directory| directory.starts_with(Path::new(SPLIT_BUILD_DIR)))
    {
        return Err(io::Error::other(format!(
            "the dependency directories should include {SPLIT_BUILD_DIR}; found {dependency_dirs:?}",
        )));
    }

    let test_support = messages
        .lines()
        .filter_map(|message| {
            test_support_rlib::cargo_artifacts::library_path_in_message(message, "test_support")
        })
        .next_back()
        .ok_or_else(|| io::Error::other("recorded Cargo JSON has no test_support artefact"))?;
    if !test_support.starts_with(Path::new(UPLIFTED_TARGET_DIR)) {
        return Err(io::Error::other(format!(
            "the test_support artefact should remain uplifted under {UPLIFTED_TARGET_DIR}; found {test_support:?}",
        )));
    }
    Ok(())
}
