//! Direct-rustc UI tests for the production `build.rs` CLI module slice.
//!
//! The fixtures compile the production CLI modules and their direct support
//! graph, rather than declaration-only stand-ins. A small source assertion
//! keeps their composition root aligned with the inline `cli` module in
//! `build.rs`.

#[path = "support/cargo_artifacts.rs"]
mod cargo_artifacts;
#[path = "support/rustc_response_file.rs"]
mod rustc_response_file;

use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// The external crates required by the real build-script CLI slice.
const REQUIRED_EXTERNS: &[&str] = &["clap", "ortho_config", "serde", "thiserror", "tracing"];

/// The exact module declarations that the fixture mirrors from `build.rs`.
const BUILD_SLICE_MODULES: &[(&str, &str)] = &[
    ("config.rs", "pub mod config;"),
    ("validation.rs", "mod validation;"),
    ("help.rs", "mod help;"),
    ("command.rs", "mod command;"),
];

/// Verify the production build-script module root and its runtime boundary.
#[test]
fn production_build_module_slice_has_expected_boundary() -> io::Result<()> {
    assert_fixture_matches_build_rs()?;
    let dependencies = BuildSliceDependencies::build()?;
    let supported = dependencies.compile("tests/ui/build_module_slice_supported.rs")?;
    if !supported.status.success() {
        return Err(io::Error::other(format!(
            "the supported build module slice should compile:\n{}",
            stderr(&supported),
        )));
    }

    let runtime_import =
        dependencies.compile("tests/ui/build_module_slice_runtime_module_fail.rs")?;
    let standard_error = stderr(&runtime_import);

    if runtime_import.status.success() {
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

/// The direct-rustc dependencies used by the production module fixtures.
struct BuildSliceDependencies {
    /// Extern crates explicitly imported by the production module paths.
    externs: Vec<(&'static str, PathBuf)>,
    /// Directories containing transitive artefacts rustc resolves from metadata.
    dependency_dirs: Vec<PathBuf>,
}

impl BuildSliceDependencies {
    /// Build the crate and collect the artefacts required by the UI fixtures.
    fn build() -> io::Result<Self> {
        let output = Command::new(cargo())
            .arg("build")
            .arg("--lib")
            .arg("--manifest-path")
            .arg(manifest_dir().join("Cargo.toml"))
            .arg("--message-format=json")
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "building the production module dependencies failed:\n{}",
                stderr(&output),
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let externs = REQUIRED_EXTERNS
            .iter()
            .map(|name| {
                let artefact = stdout
                    .lines()
                    .filter_map(|line| cargo_artifacts::library_path_in_message(line, name))
                    .next_back()
                    .ok_or_else(|| {
                        io::Error::other(format!(
                            "cargo reported no {name} artefact for the build-slice fixture"
                        ))
                    })?;
                Ok((*name, artefact))
            })
            .collect::<io::Result<Vec<_>>>()?;
        let mut dependency_dirs = Vec::new();
        for parent in stdout
            .lines()
            .flat_map(cargo_artifacts::dependency_dirs_in_message)
        {
            if !dependency_dirs.contains(&parent) {
                dependency_dirs.push(parent);
            }
        }
        if dependency_dirs.is_empty() {
            return Err(io::Error::other(
                "cargo reported no dependency artefact directories for the build-slice fixture",
            ));
        }

        Ok(Self {
            externs,
            dependency_dirs,
        })
    }

    /// Compile one fixture against the production modules with the workspace rustc.
    fn compile(&self, source: &str) -> io::Result<Output> {
        let output_dir = tempfile::tempdir_in(manifest_dir().join("target"))?;
        let mut args = vec![
            String::from("--edition=2024"),
            String::from("--crate-type=bin"),
            String::from("--emit=metadata"),
            manifest_dir().join(source).to_string_lossy().into_owned(),
        ];
        args.extend(self.externs.iter().flat_map(|(name, path)| {
            [
                String::from("--extern"),
                format!("{name}={}", path.display()),
            ]
        }));
        args.extend(
            self.dependency_dirs
                .iter()
                .flat_map(|path| [String::from("-L"), format!("dependency={}", path.display())]),
        );
        args.extend([
            String::from("-o"),
            output_dir
                .path()
                .join("build-module-slice-ui.rmeta")
                .to_string_lossy()
                .into_owned(),
        ]);

        let response =
            rustc_response_file::write(output_dir.path(), "build-module-slice-ui.args", &args)?;
        let mut command = Command::new(rustc());
        command.arg(response).envs(package_environment());
        command.output()
    }
}

/// Verify the fixture root still mirrors the module declarations in `build.rs`.
fn assert_fixture_matches_build_rs() -> io::Result<()> {
    let build_script = test_support::fs::read_to_string(manifest_dir().join("build.rs"))?;
    let slice_start = build_script
        .find("#[path = \"src/cli\"]\nmod cli {")
        .ok_or_else(|| io::Error::other("build.rs no longer declares its inline cli module"))?;
    let declared_slice = build_script
        .get(slice_start..)
        .and_then(|slice| {
            slice
                .find("#[path = \"src/cli_localization.rs\"]")
                .and_then(|slice_end| slice.get(..slice_end))
        })
        .ok_or_else(|| {
            io::Error::other("could not locate the end of build.rs's cli module slice")
        })?;

    for (path, declaration) in BUILD_SLICE_MODULES {
        let expected = format!("#[path = \"{path}\"]\n    {declaration}");
        if !declared_slice.contains(&expected) {
            return Err(io::Error::other(format!(
                "build.rs's cli slice no longer matches the UI fixture: missing {expected:?}",
            )));
        }
    }
    if declared_slice.matches("#[path = ").count() != BUILD_SLICE_MODULES.len() + 1 {
        return Err(io::Error::other(
            "build.rs's cli slice contains a different set of path modules than the UI fixture",
        ));
    }
    Ok(())
}

/// Supply the Cargo package variables consumed by Clap's command derives.
const fn package_environment() -> [(&'static str, &'static str); 7] {
    [
        ("CARGO_PKG_NAME", env!("CARGO_PKG_NAME")),
        ("CARGO_PKG_VERSION", env!("CARGO_PKG_VERSION")),
        ("CARGO_PKG_AUTHORS", env!("CARGO_PKG_AUTHORS")),
        ("CARGO_PKG_DESCRIPTION", env!("CARGO_PKG_DESCRIPTION")),
        ("CARGO_PKG_HOMEPAGE", env!("CARGO_PKG_HOMEPAGE")),
        ("CARGO_PKG_REPOSITORY", env!("CARGO_PKG_REPOSITORY")),
        ("CARGO_PKG_LICENSE", env!("CARGO_PKG_LICENSE")),
    ]
}

/// Return the repository root supplied by Cargo for this test target.
fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Return the Cargo executable selected for the workspace.
#[expect(
    clippy::disallowed_methods,
    reason = "Cargo supplies the executable path for direct UI compilation; the test only reads the tool location"
)]
fn cargo() -> PathBuf {
    std::env::var_os("CARGO").map_or_else(|| Path::new("cargo").to_path_buf(), PathBuf::from)
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
