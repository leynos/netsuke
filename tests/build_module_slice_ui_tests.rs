//! Direct-rustc UI tests for the `build.rs` CLI module slice.
//!
//! The fixtures mirror the inline `cli` composition root without compiling the
//! production modules. This keeps the negative case dependency-free while
//! making the declared-module boundary an explicit compiler contract.

use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// Verify the supported build-script module root compiles directly.
#[test]
fn supported_build_module_slice_compiles() -> io::Result<()> {
    let output = compile_ui_fixture("tests/ui/build_module_slice_supported.rs")?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "the supported build module slice should compile:\n{}",
            stderr(&output),
        )));
    }
    Ok(())
}

/// Verify runtime-only CLI modules remain absent from the build-script slice.
#[test]
fn runtime_module_import_is_rejected_by_the_build_module_slice() -> io::Result<()> {
    let output = compile_ui_fixture("tests/ui/build_module_slice_runtime_module_fail.rs")?;
    let standard_error = stderr(&output);

    if output.status.success() {
        return Err(io::Error::other(
            "the build module slice should reject a runtime-only module import",
        ));
    }
    if !standard_error.contains("discovery") {
        return Err(io::Error::other(format!(
            "the compiler diagnostic should identify discovery as missing:\n{standard_error}",
        )));
    }
    if !standard_error.contains("unresolved import") && !standard_error.contains("could not find") {
        return Err(io::Error::other(format!(
            "the compiler diagnostic should explain the unresolved module:\n{standard_error}",
        )));
    }
    Ok(())
}

/// Compile one dependency-free module-slice fixture with the workspace rustc.
fn compile_ui_fixture(source: &str) -> io::Result<Output> {
    let output_dir = tempfile::tempdir_in(manifest_dir().join("target"))?;

    Command::new(rustc())
        .arg("--edition=2024")
        .arg("--crate-type=bin")
        .arg("--emit=metadata")
        .arg(manifest_dir().join(source))
        .arg("-o")
        .arg(output_dir.path().join("build-module-slice-ui.rmeta"))
        .output()
}

/// Return the repository root supplied by Cargo for this test target.
fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Return the rustc executable selected for the workspace.
#[expect(
    clippy::disallowed_methods,
    reason = "Cargo supplies the rustc path for direct UI compilation; the test only reads the tool location"
)]
fn rustc() -> PathBuf {
    std::env::var_os("RUSTC").map_or_else(|| Path::new("rustc").to_path_buf(), PathBuf::from)
}

/// Render a compiler invocation's standard error for a test failure.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
