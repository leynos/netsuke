//! Builds the `test_support` rlib for direct-rustc UI harnesses.
//!
//! `test_support` is a dev-dependency of this package, so a fixture that
//! `rustc` compiles directly needs Cargo to report where the uplifted rlib and
//! each of its dependency directories landed. Cargo 1.99 gives every crate its
//! own artefact directory, so the search paths come from Cargo's JSON messages
//! rather than an assumed shared `target/<profile>/deps`.
//!
//! The rlib itself is built by Cargo, so it is borrow-checked exactly as the
//! rest of the suite is; only the fixtures are compiled directly.
//!
//! Reuse policy: include this module from `tests/*.rs` direct-rustc UI
//! harnesses that compile fixtures against `test_support`. Harnesses compiling
//! fixtures against the production crate, or against no crate at all, do not
//! need it.

#[path = "cargo_artifacts.rs"]
pub(crate) mod cargo_artifacts;
#[path = "cargo_features.rs"]
mod cargo_features;
#[path = "rustc_response_file.rs"]
mod rustc_response_file;

use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::Instant,
};

/// The `test_support` rlib and the directories holding its dependencies.
pub struct TestSupportRlib {
    /// The metadata artefact the fixtures load through `--extern`.
    pub rlib: PathBuf,
    /// Directories passed to `rustc` through `-L dependency=`.
    pub deps_dirs: Vec<PathBuf>,
}

impl TestSupportRlib {
    /// Build `test_support` with Cargo and locate the resulting rlib.
    pub fn build() -> io::Result<Self> {
        let mut command = Command::new(cargo());
        command
            .arg("build")
            .arg("--manifest-path")
            .arg(manifest_dir().join("test_support/Cargo.toml"))
            .arg("--message-format=json")
            .args(cargo_features::GATE_FEATURE_ARGUMENTS);
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
    /// so `deps_dirs` holds one entry per dependency, and a harness may add
    /// long temporary roots on top; passed directly, the result exceeds the
    /// Windows `CreateProcess` command-line limit and the spawn fails with
    /// `Os { code: 206 }` before `rustc` runs. Every directory is required to
    /// avoid `E0463`, so the list moves off the command line rather than being
    /// shortened.
    pub fn compile(&self, source: &str) -> io::Result<Output> {
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
                .join("test-support-ui.rmeta")
                .to_string_lossy()
                .into_owned(),
        );

        let response =
            rustc_response_file::write(output_dir.path(), "test-support-ui.args", &args)?;
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

/// Locate the workspace root containing the compile-time fixtures.
pub fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Locate the Cargo that built the calling test.
#[expect(
    clippy::disallowed_methods,
    reason = "locating build tools Cargo reports through the environment; there is no seam to inject and no process state to isolate"
)]
pub fn cargo() -> PathBuf {
    std::env::var_os("CARGO").map_or_else(|| Path::new("cargo").to_path_buf(), PathBuf::from)
}

/// Locate the Rust compiler selected for the workspace toolchain.
#[expect(
    clippy::disallowed_methods,
    reason = "locating build tools Cargo reports through the environment; there is no seam to inject and no process state to isolate"
)]
pub fn rustc() -> PathBuf {
    std::env::var_os("RUSTC").map_or_else(|| Path::new("rustc").to_path_buf(), PathBuf::from)
}

/// Render `output`'s standard error as lossy UTF-8 for an assertion message.
#[must_use]
pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
