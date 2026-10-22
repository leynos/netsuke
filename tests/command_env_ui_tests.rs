//! Compile-time tests for public environment-injection APIs.
//!
//! The fixture in `tests/ui/command_env_embedder_pass.rs` imports and
//! constructs `CommandEnv`, `NinjaBuildRequest`, and `NinjaToolRequest`, and
//! references `run_ninja_with`/`run_ninja_tool_with`, exactly as an external
//! embedder would, so a visibility or signature regression fails this suite
//! rather than only the crate's own tests.
//! The cached CLI configuration fixture exercises the equivalent public
//! boundary for `ConfigEnvProvider` and `DiscoveredLayers` through the same
//! artefact-reuse harness, avoiding a separate dependency resolution and build
//! inside the timed test.
//!
//! There is deliberately no compile-fail case for the removed APIs
//! (`EnvMut`, `PathGuard`, `prepend_dir_to_path`, `override_ninja_env`): the
//! workspace build already rejects any revived call site, and pinning rustc's
//! diagnostic wording for a missing item would make the suite fail on
//! compiler upgrades without guarding anything extra.
//!
//! Trybuild drove this during the Polonius migration and could not: it removes
//! ambient `RUSTFLAGS` and overrides workspace `build.rustflags` outright, so
//! while Polonius was flag-gated it rebuilt the `netsuke` dependency without
//! the analysis and rejected the crate's `POLONIUS(...)` sites (see
//! docs/polonius.md). The pinned nightly now enables Polonius by default, so
//! that hazard is gone, but the direct-compile harness is kept: it needs no
//! scratch project and no toolchain-sensitive `.stderr` snapshot. Cargo check
//! supplies the `netsuke` metadata artefact, and the fixture is compiled
//! directly with the workspace `rustc` against it.

#[path = "support/cargo_artifacts.rs"]
mod cargo_artifacts;
#[path = "support/cargo_features.rs"]
mod cargo_features;
#[path = "support/rustc_response_file.rs"]
mod rustc_response_file;

use rstest::rstest;
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// The embedder fixture type-checks against the public API.
#[test]
fn command_env_embedder_fixture_compiles() -> io::Result<()> {
    compile_public_api_fixture(
        "tests/ui/command_env_embedder_pass.rs",
        "the embedder fixture should compile against the public API",
    )
}

/// The manifest-environment fixture type-checks against the public API.
#[test]
fn manifest_environment_embedder_fixture_compiles() -> io::Result<()> {
    compile_public_api_fixture(
        "tests/ui/manifest_environment_embedder_pass.rs",
        "the manifest-environment fixture should compile against the public API",
    )
}

/// Verifies external callers can parse policy types without depending on Clap.
#[test]
fn policy_from_str_embedder_fixture_compiles_without_clap() -> io::Result<()> {
    compile_public_api_fixture(
        "tests/ui/policy_from_str_embedder_pass.rs",
        "the policy FromStr fixture should compile without Clap",
    )
}
/// The public lint API fixture compiles for an external embedder.
#[cfg(feature = "lint")]
#[test]
fn lint_public_api_fixture_compiles() -> io::Result<()> {
    compile_public_api_fixture(
        "tests/ui/lint_public_api_pass.rs",
        "the lint public API fixture should compile",
    )
}

/// Type-check the public cache API only when the Netsuke library is supplied.
#[rstest]
#[case::public_api(true)]
#[case::missing_library(false)]
fn cli_configuration_fixture_compiles(#[case] has_netsuke_extern: bool) -> io::Result<()> {
    let library = NetsukeRlib::build()?;
    let output = library.compile("tests/ui/cli_configuration_pass.rs", has_netsuke_extern)?;
    if output.status.success() != has_netsuke_extern {
        return Err(io::Error::other(format!(
            "the CLI configuration fixture must compile exactly when Netsuke is supplied:\n{}",
            stderr(&output),
        )));
    }
    Ok(())
}
/// The public command-list constructors compile for an external embedder.
#[test]
fn command_list_public_api_fixture_compiles() -> io::Result<()> {
    compile_public_api_fixture(
        "tests/ui/command_list_public_api_pass.rs",
        "the command-list public API fixture should compile",
    )
}

/// The public IR-generation error fixture compiles for an external embedder.
#[test]
fn ir_gen_error_public_api_fixture_compiles() -> io::Result<()> {
    compile_public_api_fixture(
        "tests/ui/ir_gen_error_public_api_pass.rs",
        "the IR-generation error public API fixture should compile",
    )
}

/// The public legacy-recipe metric constants compile for an external embedder.
#[test]
fn legacy_recipe_telemetry_public_api_fixture_compiles() -> io::Result<()> {
    compile_public_api_fixture(
        "tests/ui/legacy_recipe_telemetry_public_api_pass.rs",
        "the legacy-recipe telemetry fixture should compile against the public API",
    )
}

/// The cached configuration-discovery API compiles for an external embedder.
#[test]
fn config_cached_discovery_embedder_fixture_compiles() -> io::Result<()> {
    compile_public_api_fixture(
        "tests/ui/config_cached_discovery_embedder_pass.rs",
        "the cached configuration-discovery fixture should compile against the public API",
    )
}

/// The verbose timing constructors compile for an external embedder.
#[test]
fn verbose_timing_reporter_embedder_fixture_compiles() -> io::Result<()> {
    let source = "tests/ui/verbose_timing_reporter_embedder_pass.rs";
    let rlib = NetsukeRlib::build()?;
    let output = rlib.compile(source, true)?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "the verbose timing reporter fixture should compile against the public API:\n{}",
            stderr(&output)
        )));
    }

    let control = rlib.compile(source, false)?;
    if control.status.success() {
        return Err(io::Error::other(
            "the verbose timing fixture compiled without --extern netsuke",
        ));
    }
    Ok(())
}
/// Compile one external public-API fixture through the direct-rustc harness.
fn compile_public_api_fixture(source: &str, failure_message: &str) -> io::Result<()> {
    let rlib = NetsukeRlib::build()?;
    let output = rlib.compile(source, true)?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{failure_message}:\n{}",
            stderr(&output)
        )));
    }
    Ok(())
}

/// The `netsuke` metadata artefact and every directory holding its dependencies.
struct NetsukeRlib {
    rlib: PathBuf,
    deps_dirs: Vec<PathBuf>,
}

impl NetsukeRlib {
    /// Check the production library and locate the `netsuke` metadata artefact.
    ///
    /// Cargo inherits the ambient `RUSTFLAGS` and pinned toolchain. This checks
    /// `netsuke-build --lib --all-features`, including its build script and
    /// build dependencies, without checking unrelated workspace test targets.
    /// Check mode avoids linking integration executables and does not promise
    /// the same test build mode or cache identity. The package is named
    /// `netsuke-build`, but the lib target — and therefore the `--extern` name
    /// — is `netsuke`.
    fn build() -> io::Result<Self> {
        let output = Command::new(cargo())
            .arg("check")
            .arg("--manifest-path")
            .arg(manifest_dir().join("Cargo.toml"))
            .arg("--message-format=json-render-diagnostics")
            .args(["--package", "netsuke-build"])
            .args(cargo_features::LIBRARY_BUILD_ARGUMENTS)
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "checking the netsuke library failed:\n{}",
                stderr(&output),
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let rlib = cargo_artifacts::library_path_in_messages(
            &stdout,
            cargo_artifacts::LibraryArtefactSelection {
                target_name: "netsuke",
                expected_package_id: None,
            },
        )?;
        // Dependencies do not sit in one predictable directory. Cargo uplifts
        // the top-level package's artefacts into the profile directory, and
        // the Cargo shipped with the 1.99 nightlies gives every crate its own
        // build directory rather than one shared `deps/`. Each
        // compiler-artifact message names where its own artefacts really
        // landed, so derive the search path from what Cargo reports.
        let mut deps_dirs: Vec<PathBuf> = Vec::new();
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
                "cargo reported no library artefacts to derive dependency dirs from",
            ));
        }
        Ok(Self { rlib, deps_dirs })
    }

    /// Type-check `source` with or without the `netsuke` metadata artefact.
    ///
    /// `--emit=metadata` is enough to surface any visibility or signature
    /// regression while sparing the harness a full link of `netsuke`'s
    /// dependency tree. When `include_netsuke_extern` is `false`, the fixture
    /// must fail because it imports `netsuke`; this control proves the normal
    /// compile path receives an effective `--extern` argument.
    ///
    /// The arguments travel in a `rustc` response file rather than on the
    /// command line. Cargo 1.99 gives every crate its own artefact directory,
    /// so `deps_dirs` holds one entry per dependency; passed directly, a list
    /// that long can exceed the Windows `CreateProcess` command-line limit and
    /// fail the spawn with `Os { code: 206 }` before `rustc` runs. Every
    /// directory is required to avoid `E0463`, so the list moves off the
    /// command line rather than being shortened.
    fn compile(&self, source: &str, include_netsuke_extern: bool) -> io::Result<Output> {
        let output_dir = tempfile::tempdir()?;
        let mut args = vec![
            String::from("--edition=2024"),
            String::from("--crate-type=bin"),
            String::from("--emit=metadata"),
            manifest_dir().join(source).to_string_lossy().into_owned(),
        ];

        if include_netsuke_extern {
            args.extend([
                String::from("--extern"),
                format!("netsuke={}", self.rlib.display()),
            ]);
        }

        args.extend(
            self.deps_dirs
                .iter()
                .flat_map(|dir| [String::from("-L"), format!("dependency={}", dir.display())]),
        );
        args.push(String::from("-o"));
        args.push(
            output_dir
                .path()
                .join("command-env-ui.rmeta")
                .to_string_lossy()
                .into_owned(),
        );

        let response = rustc_response_file::write(output_dir.path(), "command-env-ui.args", &args)?;
        // `output_dir` owns the response file and stays in scope across the
        // call below, so the file still exists when `rustc` opens it at spawn.
        rustc().arg(response).output()
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

/// Construct the fixture compiler command from Cargo's toolchain environment.
#[expect(
    clippy::disallowed_methods,
    reason = "thin adapter reads Cargo's toolchain environment; command construction is tested with injected values"
)]
fn rustc() -> Command {
    rustc_command_with(|key| std::env::var_os(key))
}

/// Construct a compiler command from injected toolchain environment values.
///
/// For `RUSTC=chosen-rustc` and `RUSTC_WRAPPER=build-limits-rustc`, the command
/// starts with `build-limits-rustc chosen-rustc`. An empty wrapper disables
/// wrapping, matching Cargo's convention. Only this harness's adapter and
/// command-construction tests may call this helper.
fn rustc_command_with(get_env: impl Fn(&str) -> Option<OsString>) -> Command {
    let compiler = get_env("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let wrapper = get_env("RUSTC_WRAPPER").filter(|path| !path.is_empty());
    if let Some(wrapper_path) = wrapper {
        let mut command = Command::new(wrapper_path);
        command.arg(compiler);
        command
    } else {
        Command::new(compiler)
    }
}

#[rstest]
#[case::default_compiler(None, None, ("rustc", vec![]))]
#[case::selected_compiler(Some("chosen rustc"), None, ("chosen rustc", vec![]))]
#[case::wrapper(Some("chosen rustc"), Some("build-limits-rustc"), ("build-limits-rustc", vec!["chosen rustc"]))]
#[case::wrapper_with_default_compiler(None, Some("build-limits-rustc"), ("build-limits-rustc", vec!["rustc"]))]
#[case::empty_wrapper(Some("chosen rustc"), Some(""), ("chosen rustc", vec![]))]
fn fixture_compiler_command_honors_cargo_wrapper(
    #[case] compiler: Option<&str>,
    #[case] wrapper: Option<&str>,
    #[case] expected: (&str, Vec<&str>),
) {
    let command = rustc_command_with(|key| match key {
        "RUSTC" => compiler.map(OsString::from),
        "RUSTC_WRAPPER" => wrapper.map(OsString::from),
        _ => None,
    });
    assert_eq!(command.get_program(), Path::new(expected.0).as_os_str());
    assert_eq!(
        command.get_args().map(OsString::from).collect::<Vec<_>>(),
        expected
            .1
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>(),
    );
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
