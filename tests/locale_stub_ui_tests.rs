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

#[path = "support/cargo_artifacts.rs"]
mod cargo_artifacts;
#[path = "support/rustc_response_file.rs"]
mod rustc_response_file;

use rstest::{fixture, rstest};
use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::Instant,
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
    rlib: PathBuf,
    deps_dirs: Vec<PathBuf>,
}

impl TestSupportRlib {
    /// Build `test_support` with Cargo and locate the resulting rlib.
    ///
    /// Cargo inherits the ambient `RUSTFLAGS` and the pinned toolchain, so the
    /// rlib is borrow-checked exactly as the rest of the suite is.
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
        let started_at = Instant::now();
        let output = command.output()?;
        tracing::info!(
            elapsed_seconds = started_at.elapsed().as_secs_f64(),
            "cargo build test_support completed"
        );
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "building test_support failed:\n{}",
                stderr(&output),
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let rlib = stdout
            .lines()
            .filter_map(|line| cargo_artifacts::library_path_in_message(line, "test_support"))
            .next_back()
            .ok_or_else(|| io::Error::other("cargo reported no test_support rlib artefact"))?;
        // Dependencies do not necessarily sit beside the uplifted
        // `test_support` rlib: Cargo's `build.build-dir` setting splits
        // intermediate artefacts (where dependencies live) from final ones,
        // and the Cargo shipped with the 1.99 nightlies gives every crate its
        // own directory rather than one shared `deps/`. Every
        // compiler-artifact message names where its own artefacts really
        // landed, so collect each parent directory for `-L dependency=`.
        let mut deps_dirs = Vec::new();
        for parent in stdout
            .lines()
            .flat_map(cargo_artifacts::dependency_dirs_in_message)
        {
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
    ///
    /// The arguments travel in a `rustc` response file rather than on the
    /// command line. Cargo 1.99 gives every crate its own artefact directory,
    /// so `deps_dirs` holds one entry per dependency, and the split-build test
    /// adds long temporary roots on top; passed directly, the result exceeds
    /// the Windows `CreateProcess` command-line limit and the spawn fails with
    /// `Os { code: 206 }` before `rustc` runs. Every directory is required to
    /// avoid `E0463`, so the list moves off the command line rather than being
    /// shortened.
    fn compile(&self, source: &str) -> io::Result<Output> {
        let output_dir = tempfile::tempdir()?;
        let started_at = Instant::now();
        let mut args = vec![
            String::from("--edition=2024"),
            String::from("--crate-type=bin"),
            String::from("--emit=metadata"),
            manifest_dir().join(source).to_string_lossy().into_owned(),
            String::from("--extern"),
            format!("test_support={}", self.rlib.display()),
        ];
        args.extend(
            self.deps_dirs
                .iter()
                .flat_map(|dir| [String::from("-L"), format!("dependency={}", dir.display())]),
        );
        args.push(String::from("-o"));
        args.push(
            output_dir
                .path()
                .join("stub-env-ui.rmeta")
                .to_string_lossy()
                .into_owned(),
        );

        let response = rustc_response_file::write(output_dir.path(), "stub-env-ui.args", &args)?;
        // `output_dir` owns the response file and stays in scope across the
        // call below, so the file still exists when `rustc` opens it at spawn.
        let output = Command::new(rustc()).arg(response).output()?;
        tracing::info!(
            elapsed_seconds = started_at.elapsed().as_secs_f64(),
            "rustc metadata harness completed"
        );
        Ok(output)
    }
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

/// Forcing a split `build.build-dir` must still yield a working harness:
/// the dependency rlibs land apart from the uplifted `test_support` rlib, so
/// the collected `-L dependency=` set has to span the split for the control
/// fixture to compile. This pins the regression where a single derived
/// directory missed the dependencies entirely.
#[rstest]
fn harness_compiles_under_a_split_build_dir() -> io::Result<()> {
    // A full `cargo test` run may already have a test subscriber, while
    // nextest needs this one to surface the scoped timing events.
    drop(tracing_subscriber::fmt().with_test_writer().try_init());
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
