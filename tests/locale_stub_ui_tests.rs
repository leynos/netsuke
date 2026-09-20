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
use std::{io, path::Path, process::Command};
use test_support::fs as test_fs;

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

#[rstest]
fn split_build_fixture_compiles_through_the_direct_rustc_harness() -> io::Result<()> {
    let workspace = tempfile::tempdir()?;
    let dependency = workspace.path().join("fixture_dependency");
    let support = workspace.path().join("fixture_support");
    let target_dir = workspace.path().join("target");
    let build_dir = workspace.path().join("build");
    test_fs::create_dir_all(dependency.join("src"))?;
    test_fs::create_dir_all(support.join("src"))?;
    test_fs::write(
        workspace.path().join("Cargo.toml"),
        concat!(
            "[workspace]\n",
            "members = [\"fixture_dependency\", \"fixture_support\"]\n",
            "resolver = \"3\"\n",
        ),
    )?;
    test_fs::write(
        dependency.join("Cargo.toml"),
        concat!(
            "[package]\n",
            "name = \"fixture_dependency\"\n",
            "version = \"0.1.0\"\n",
            "edition = \"2024\"\n",
        ),
    )?;
    test_fs::write(
        dependency.join("src/lib.rs"),
        "pub fn answer() -> u8 { 42 }\n",
    )?;
    test_fs::write(
        support.join("Cargo.toml"),
        concat!(
            "[package]\n",
            "name = \"fixture_support\"\n",
            "version = \"0.1.0\"\n",
            "edition = \"2024\"\n\n",
            "[dependencies]\n",
            "fixture_dependency = { path = \"../fixture_dependency\" }\n",
        ),
    )?;
    test_fs::write(
        support.join("src/lib.rs"),
        "pub fn answer() -> u8 { fixture_dependency::answer() }\n",
    )?;

    let cargo = Command::new(test_support_rlib::cargo())
        .current_dir(workspace.path())
        .arg("build")
        .arg("--manifest-path")
        .arg(support.join("Cargo.toml"))
        .arg("--message-format=json")
        .env("CARGO_TARGET_DIR", &target_dir)
        .env("CARGO_BUILD_BUILD_DIR", &build_dir)
        .output()?;
    if !cargo.status.success() {
        return Err(io::Error::other(format!(
            "the split-build fixture Cargo build should succeed:\n{}",
            test_support_rlib::stderr(&cargo),
        )));
    }

    let messages = String::from_utf8_lossy(&cargo.stdout);
    let support_artefact = messages
        .lines()
        .filter_map(|message| {
            test_support_rlib::cargo_artifacts::library_path_in_message(message, "fixture_support")
        })
        .next_back()
        .ok_or_else(|| io::Error::other("Cargo reported no fixture_support rlib"))?;
    let mut dependency_dirs = Vec::new();
    for directory in messages
        .lines()
        .flat_map(test_support_rlib::cargo_artifacts::dependency_dirs_in_message)
    {
        if !dependency_dirs.contains(&directory) {
            dependency_dirs.push(directory);
        }
    }
    if !dependency_dirs
        .iter()
        .any(|directory| directory.starts_with(&build_dir))
    {
        return Err(io::Error::other(format!(
            "the dependency directories should include {}; found {dependency_dirs:?}",
            build_dir.display(),
        )));
    }

    let source = workspace.path().join("main.rs");
    test_fs::write(
        &source,
        "fn main() { assert_eq!(fixture_support::answer(), 42); }\n",
    )?;
    let mut args = vec![
        String::from("--edition=2024"),
        String::from("--crate-type=bin"),
        String::from("--emit=metadata"),
        source.to_string_lossy().into_owned(),
        String::from("--extern"),
        format!("fixture_support={}", support_artefact.display()),
    ];
    args.extend(dependency_dirs.iter().flat_map(|directory| {
        [
            String::from("-L"),
            format!("dependency={}", directory.display()),
        ]
    }));
    args.push(String::from("-o"));
    args.push(
        workspace
            .path()
            .join("split-build-fixture.rmeta")
            .to_string_lossy()
            .into_owned(),
    );
    let response = test_support_rlib::rustc_response_file::write(
        workspace.path(),
        "split-build-fixture.args",
        &args,
    )?;
    let rustc = Command::new(test_support_rlib::rustc())
        .current_dir(workspace.path())
        .arg(response)
        .output()?;
    if !rustc.status.success() {
        return Err(io::Error::other(format!(
            "the direct-rustc fixture should compile through the response file:\n{}",
            test_support_rlib::stderr(&rustc),
        )));
    }
    Ok(())
}
