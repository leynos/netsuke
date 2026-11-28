//! Compile checked-in `tests/ui` fixtures with the workspace compiler.
//!
//! The compile-time suites all need the same invocation: `rustc` from the
//! workspace toolchain, a bounded deadline, and diagnostics decoded for the
//! assertion message. Keeping that here means a change to the invocation is
//! made once rather than in every suite, and the suites only describe which
//! fixture they compile and which rejection they expect.

use crate::bounded_process::{COMPILE_DEADLINE, output_within};
use mockable::{DefaultEnv, Env};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Compile `source` into `output_path`, adding `extra_args` to the invocation.
///
/// `source` is an absolute path: the caller joins its own manifest directory,
/// which differs from this crate's. Extra arguments come after the crate type
/// so a suite can add its own `--cfg` and check-cfg policy.
///
/// # Errors
///
/// Returns an error when the compiler cannot be spawned or exceeds its
/// deadline.
pub fn compile_fixture(
    source: &Path,
    output_path: &Path,
    extra_args: &[&str],
) -> io::Result<Output> {
    // nested-cargo-allow: timed the spawn is bounded by `output_within`'s deadline
    let mut compile = Command::new(rustc());
    compile
        .arg("--edition=2024")
        .arg("--crate-type=bin")
        .args(extra_args)
        .arg(source)
        .arg("-o")
        .arg(output_path);
    output_within(&mut compile, COMPILE_DEADLINE)
}

/// Decode compiler diagnostics for assertion messages.
#[must_use]
pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Locate the Rust compiler selected for the workspace toolchain.
///
/// `RUSTC` is read through the injected environment seam rather than the
/// process environment, so this helper follows the same policy as every other
/// test-support module that locates a compiler.
fn rustc() -> PathBuf {
    rust_compiler(&DefaultEnv)
}

/// Return the configured `RUSTC` path, defaulting to `rustc` on `PATH`.
fn rust_compiler(env: &impl Env) -> PathBuf {
    env.os_string("RUSTC")
        .map_or_else(|| Path::new("rustc").to_path_buf(), PathBuf::from)
}
