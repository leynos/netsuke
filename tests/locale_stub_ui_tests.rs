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

/// The `test_support` rlib and the deps directory holding its dependencies.
struct TestSupportRlib {
    rlib: PathBuf,
    deps_dir: PathBuf,
}

impl TestSupportRlib {
    /// Build `test_support` with Cargo and locate the resulting rlib.
    ///
    /// Cargo inherits the ambient `RUSTFLAGS`, so the rlib is borrow-checked
    /// with the same Polonius flags as the rest of the suite — the property
    /// trybuild could not preserve.
    fn build() -> io::Result<Self> {
        let output = Command::new(cargo())
            .arg("build")
            .arg("--manifest-path")
            .arg(manifest_dir().join("test_support/Cargo.toml"))
            .arg("--message-format=json")
            .output()?;
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
        // Cargo uplifts the top-level package's rlib out of `deps/` into the
        // profile directory, so the rlib's own parent is not where the
        // dependency rlibs live.
        let parent = rlib
            .parent()
            .ok_or_else(|| io::Error::other("the rlib path should have a parent"))?;
        let deps_dir = if parent.file_name() == Some(std::ffi::OsStr::new("deps")) {
            parent.to_path_buf()
        } else {
            parent.join("deps")
        };
        Ok(Self { rlib, deps_dir })
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
            .arg(format!("test_support={}", self.rlib.display()))
            .arg("-L")
            .arg(format!("dependency={}", self.deps_dir.display()))
            .arg("-o")
            .arg(output_dir.path().join("stub-env-ui.rmeta"))
            .output()
    }
}

/// Extract the `test_support` rlib path from one Cargo JSON message, if any.
fn test_support_rlib_in_message(line: &str) -> Option<PathBuf> {
    let message: serde_json::Value = serde_json::from_str(line).ok()?;
    if message.get("reason")? != "compiler-artifact"
        || message.get("target")?.get("name")? != "test_support"
    {
        return None;
    }
    message
        .get("filenames")?
        .as_array()?
        .iter()
        .filter_map(|filename| filename.as_str())
        .filter(|filename| {
            Path::new(filename)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("rlib"))
        })
        .map(PathBuf::from)
        .next_back()
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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
