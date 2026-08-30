//! Compile-time regression tests for the Ninja escaping ownership boundary.

use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// Verify the consuming conversion rejects a second escape attempt at compile time.
#[test]
fn ninja_value_cannot_cross_the_escape_boundary_twice() -> io::Result<()> {
    let output_dir = tempfile::tempdir()?;
    let output = compile_ui_fixture(
        "tests/ui/ninja_gen_escape_double_escape_compile_fail.rs",
        &output_dir.path().join("ninja-gen-escape-ui"),
    )?;

    if output.status.success() {
        return Err(io::Error::other(
            "a NinjaValue must not compile as input to escape_ninja_value",
        ));
    }
    let stderr = stderr(&output);
    if !is_escape_boundary_type_mismatch(&stderr) {
        return Err(io::Error::other(format!(
            "the rejection must be a ShellText/NinjaValue type mismatch:\n{stderr}",
        )));
    }
    Ok(())
}

/// Compile one escaping-boundary fixture with the workspace Rust compiler.
fn compile_ui_fixture(source: &str, output_path: &Path) -> io::Result<Output> {
    Command::new(rustc())
        .arg("--edition=2024")
        .arg("--crate-type=bin")
        .arg(manifest_dir().join(source))
        .arg("-o")
        .arg(output_path)
        .output()
}

/// Locate the workspace root containing compile-time fixtures.
fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Locate the Rust compiler selected for the workspace toolchain.
#[expect(
    clippy::disallowed_methods,
    reason = "the direct-rustc test must use Cargo's selected compiler executable"
)]
fn rustc() -> PathBuf {
    std::env::var_os("RUSTC").map_or_else(|| Path::new("rustc").to_path_buf(), PathBuf::from)
}

/// Recognize the compiler diagnostic proving the escape boundary is consuming.
fn is_escape_boundary_type_mismatch(stderr: &str) -> bool {
    ["E0308", "ShellText", "NinjaValue"]
        .into_iter()
        .all(|expected| stderr.contains(expected))
}

/// Decode compiler diagnostics for assertion messages.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
