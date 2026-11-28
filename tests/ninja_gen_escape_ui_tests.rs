//! Compile-time regression tests for the Ninja escaping ownership boundary.

use std::{io, path::PathBuf};

use test_support::ui_fixture::{compile_fixture, stderr};

/// Verify the consuming conversion rejects a second escape attempt at compile time.
#[test]
fn ninja_value_cannot_cross_the_escape_boundary_twice() -> io::Result<()> {
    let output_dir = tempfile::tempdir()?;
    let output = compile_fixture(
        &fixture("ninja_gen_escape_double_escape_compile_fail.rs"),
        &output_dir.path().join("ninja-gen-escape-ui"),
        &[],
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

/// Return the absolute path of one checked-in UI fixture.
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/ui")
        .join(name)
}

/// Recognize the compiler diagnostic proving the escape boundary is consuming.
fn is_escape_boundary_type_mismatch(stderr: &str) -> bool {
    ["E0308", "ShellText", "NinjaValue"]
        .into_iter()
        .all(|expected| stderr.contains(expected))
}
