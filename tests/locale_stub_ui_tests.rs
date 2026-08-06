//! Compile-time tests for `StubEnv`'s builder-only construction.
//!
//! `StubEnv` deliberately does not implement `Default`: on a strict stub it
//! would mean "deny every read", so `StubEnv::default()` would compile and
//! then panic at run time for the common "no locale set" case. These tests
//! keep that a compile-time contract rather than a doc-comment promise.
//!
//! Trybuild cannot drive them: it removes ambient `RUSTFLAGS` and overrides
//! workspace `build.rustflags` outright (`env_remove("RUSTFLAGS")` plus
//! `--config=build.rustflags=…` in its cargo invocations), so it would
//! rebuild the `netsuke` dependency without `-Zpolonius=next` and reject the
//! crate's `POLONIUS(...)` sites (see docs/polonius.md). Instead the
//! `test_support` rlib is built by Cargo — which does inherit the ambient
//! flags — and the fixtures are compiled directly with the workspace `rustc`
//! against that rlib.

use camino::{Utf8Path, Utf8PathBuf};
use rstest::{fixture, rstest};
use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// One `test_support` build shared by both tests.
///
/// Built once: the two tests run in parallel, so independent builds would
/// contend on Cargo's target-directory lock and repeat completed work.
#[fixture]
#[once]
fn test_support_rlib() -> TestSupportRlib {
    #[expect(
        clippy::expect_used,
        reason = "a once fixture cannot return Result; a build failure must abort the suite here"
    )]
    let rlib = TestSupportRlib::build().expect("test_support should build");
    rlib
}

#[rstest]
fn stub_env_default_does_not_compile(test_support_rlib: &TestSupportRlib) -> io::Result<()> {
    let output = test_support_rlib.compile("tests/ui/stub_env_default_compile_fail.rs")?;

    if output.status.success() {
        return Err(io::Error::other("StubEnv::default() should not compile"));
    }
    let stderr = stderr(&output);
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
    test_support_rlib: &TestSupportRlib,
) -> io::Result<()> {
    let output = test_support_rlib.compile("tests/ui/stub_env_strict_compile_pass.rs")?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "the control fixture should compile; the harness wiring is broken:\n{}",
            stderr(&output),
        )));
    }
    Ok(())
}

/// The `test_support` rlib and the directories holding its dependencies.
struct TestSupportRlib {
    rlib: Utf8PathBuf,
    deps_dirs: Vec<Utf8PathBuf>,
}

impl TestSupportRlib {
    /// Build `test_support` with Cargo and locate the resulting rlib.
    ///
    /// Cargo inherits the ambient `RUSTFLAGS`, so the rlib is borrow-checked
    /// with the same Polonius flags as the rest of the suite — the property
    /// trybuild could not preserve.
    fn build() -> io::Result<Self> {
        Self::build_with(&[])
    }

    /// Build `test_support` with additional environment variables applied.
    ///
    /// The split-layout regression test uses this to force Cargo's
    /// `build.build-dir` into a separate directory.
    fn build_with(env: &[(&str, &Path)]) -> io::Result<Self> {
        let mut command = Command::new(cargo());
        command
            .arg("build")
            .arg("--manifest-path")
            .arg(manifest_dir().join("test_support/Cargo.toml"))
            .arg("--message-format=json");
        for (key, value) in env {
            command.env(key, value);
        }
        let output = command.output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "building test_support failed:\n{}",
                stderr(&output),
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let rlib = stdout
            .lines()
            .filter_map(test_support_rlib_in_message)
            .next_back()
            .ok_or_else(|| io::Error::other("cargo reported no test_support rlib artefact"))?;
        // Dependency rlibs do not necessarily sit beside the uplifted
        // `test_support` rlib: Cargo's `build.build-dir` setting splits
        // intermediate artefacts (where dependencies live) from final ones.
        // Every compiler-artifact message names its rlib's real location, so
        // collect each artefact's parent directory for `-L dependency=`.
        let mut deps_dirs: Vec<Utf8PathBuf> = Vec::new();
        for parent in stdout.lines().flat_map(rlib_parents_in_message) {
            if !deps_dirs.contains(&parent) {
                deps_dirs.push(parent);
            }
        }
        if deps_dirs.is_empty() {
            return Err(io::Error::other(
                "cargo reported no rlib artefacts to derive dependency dirs from",
            ));
        }
        Ok(Self { rlib, deps_dirs })
    }

    /// Type-check `source` against the rlib without linking a binary.
    ///
    /// `--emit=metadata` is enough to surface the missing-item error while
    /// sparing the harness a full link of `test_support`'s dependency tree.
    fn compile(&self, source: &str) -> io::Result<Output> {
        let output_dir = tempfile::tempdir()?;
        Command::new(rustc())
            .arg("--edition=2024")
            .arg("--crate-type=bin")
            .arg("--emit=metadata")
            .arg(manifest_dir().join(source))
            .arg("--extern")
            .arg(format!("test_support={}", self.rlib))
            .args(
                self.deps_dirs
                    .iter()
                    .flat_map(|dir| [String::from("-L"), format!("dependency={dir}")]),
            )
            .arg("-o")
            .arg(output_dir.path().join("stub-env-ui.rmeta"))
            .output()
    }
}

/// Extract a compiler-artifact message's target name and rlib paths.
///
/// Returns `None` for lines that are not valid JSON, not compiler-artifact
/// messages, or that lack a target name; the rlib list may be empty for
/// artefacts that emit no rlib.
fn compiler_artifact_rlibs(line: &str) -> Option<(String, Vec<Utf8PathBuf>)> {
    let message: serde_json::Value = serde_json::from_str(line).ok()?;
    if message.get("reason")? != "compiler-artifact" {
        return None;
    }
    let name = message.get("target")?.get("name")?.as_str()?.to_owned();
    let rlibs = message
        .get("filenames")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .filter(|filename| {
            Utf8Path::new(filename)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("rlib"))
        })
        .map(Utf8PathBuf::from)
        .collect();
    Some((name, rlibs))
}

/// Extract the parent directories of every rlib in one Cargo JSON message.
fn rlib_parents_in_message(line: &str) -> Vec<Utf8PathBuf> {
    compiler_artifact_rlibs(line)
        .map(|(_name, rlibs)| {
            rlibs
                .iter()
                .filter_map(|rlib| rlib.parent().map(Utf8Path::to_path_buf))
                .collect()
        })
        .unwrap_or_default()
}

/// Extract the `test_support` rlib path from one Cargo JSON message, if any.
fn test_support_rlib_in_message(line: &str) -> Option<Utf8PathBuf> {
    let (name, rlibs) = compiler_artifact_rlibs(line)?;
    (name == "test_support")
        .then(|| rlibs.into_iter().next_back())
        .flatten()
}

fn manifest_dir() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[expect(
    clippy::disallowed_methods,
    reason = "locating build tools Cargo reports through the environment; there is no seam to inject and no process state to isolate"
)]
fn cargo() -> PathBuf {
    std::env::var_os("CARGO").map_or_else(|| Path::new("cargo").to_path_buf(), PathBuf::from)
}

#[expect(
    clippy::disallowed_methods,
    reason = "locating build tools Cargo reports through the environment; there is no seam to inject and no process state to isolate"
)]
fn rustc() -> PathBuf {
    std::env::var_os("RUSTC").map_or_else(|| Path::new("rustc").to_path_buf(), PathBuf::from)
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A synthetic Cargo message with two rlibs in different directories,
/// mirroring a split `build.build-dir` layout.
const SPLIT_LAYOUT_MESSAGE: &str = r#"{"reason":"compiler-artifact","target":{"name":"anyhow"},"filenames":["/build/debug/deps/libanyhow-1.rlib","/target/debug/libanyhow-1.rlib"]}"#;

#[rstest]
fn parser_collects_every_rlib_directory_from_a_message() {
    let parents = rlib_parents_in_message(SPLIT_LAYOUT_MESSAGE);
    assert_eq!(
        parents,
        vec![
            Utf8PathBuf::from("/build/debug/deps"),
            Utf8PathBuf::from("/target/debug"),
        ],
        "both rlib directories should be collected in message order"
    );
}

#[rstest]
#[case::malformed_json("not json at all")]
#[case::other_reason(r#"{"reason":"build-script-executed","target":{"name":"anyhow"}}"#)]
#[case::missing_target(r#"{"reason":"compiler-artifact","filenames":["/a/lib.rlib"]}"#)]
fn parser_ignores_non_artifact_messages(#[case] line: &str) {
    assert!(
        rlib_parents_in_message(line).is_empty(),
        "non-artifact input should yield no directories: {line:?}"
    );
    assert!(
        test_support_rlib_in_message(line).is_none(),
        "non-artifact input should yield no test_support rlib: {line:?}"
    );
}

#[rstest]
fn parser_selects_the_test_support_rlib_by_target_name() {
    let message = r#"{"reason":"compiler-artifact","target":{"name":"test_support"},"filenames":["/deps/libtest_support-1.rlib","/final/libtest_support.rlib"]}"#;
    assert_eq!(
        test_support_rlib_in_message(message),
        Some(Utf8PathBuf::from("/final/libtest_support.rlib")),
        "the last-listed rlib should win, matching Cargo's uplift ordering"
    );
    assert!(
        test_support_rlib_in_message(SPLIT_LAYOUT_MESSAGE).is_none(),
        "other targets' artefacts should not be mistaken for test_support"
    );
}

/// Forcing a split `build.build-dir` must still yield a working harness:
/// the dependency rlibs land apart from the uplifted `test_support` rlib, so
/// the collected `-L dependency=` set has to span the split for the control
/// fixture to compile. This pins the regression where a single derived
/// directory missed the dependencies entirely.
#[rstest]
fn harness_compiles_under_a_split_build_dir() -> io::Result<()> {
    // Both roots are private to this test: sharing the ambient target dir
    // with the concurrently building `#[once]` fixture races on the
    // uplifted rlibs and fails with version-skew errors (E0460).
    let target_dir = tempfile::tempdir()?;
    let build_dir = tempfile::tempdir()?;
    let harness = TestSupportRlib::build_with(&[
        ("CARGO_TARGET_DIR", target_dir.path()),
        ("CARGO_BUILD_BUILD_DIR", build_dir.path()),
    ])?;

    let spans_split_dir = harness
        .deps_dirs
        .iter()
        .any(|dir| dir.starts_with(build_dir.path()));
    if !spans_split_dir {
        return Err(io::Error::other(format!(
            "the dependency directories should include the split build dir {}; found {:?}",
            build_dir.path().display(),
            harness.deps_dirs,
        )));
    }

    let output = harness.compile("tests/ui/stub_env_strict_compile_pass.rs")?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "the control fixture should compile under a split build dir:
{}",
            stderr(&output),
        )));
    }
    Ok(())
}
