/// The embedder fixture type-checks against the public API.
#[test]
fn command_env_embedder_fixture_compiles() -> io::Result<()> {
    compile_public_api_fixture(
        "tests/ui/command_env_embedder_pass.rs",
        "the embedder fixture should compile against the public API",
    )
}

/// The CLI configuration fixture type-checks against the public cache API.
#[test]
fn cli_configuration_fixture_compiles() -> io::Result<()> {
    let target_dir = manifest_dir().join("target");
    test_fs::create_dir_all(&target_dir)?;
    let temporary_root = tempfile::tempdir_in(target_dir)?;
    let fixture_dir = temporary_root.path().join("cli_configuration_pass");
    test_fs::create_dir_all(fixture_dir.join("src"))?;
    test_fs::copy(
        manifest_dir().join("tests/ui/cli_configuration_pass/Cargo.toml"),
        fixture_dir.join("Cargo.toml"),
    )?;
    test_fs::copy(
        manifest_dir().join("tests/ui/cli_configuration_pass/src/main.rs"),
        fixture_dir.join("src/main.rs"),
    )?;

    let manifest = fixture_dir.join("Cargo.toml");
    let output = Command::new(cargo())
        .arg("check")
        .arg("--manifest-path")
        .arg(manifest)
        .env(
            "CARGO_TARGET_DIR",
            manifest_dir().join("target/cli-configuration-ui"),
        )
        .output()?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "the CLI configuration fixture should compile against the public API:\n{}",
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

/// The `netsuke` rlib and every directory holding its dependencies.
struct NetsukeRlib {
    rlib: PathBuf,
    deps_dirs: Vec<PathBuf>,
}

impl NetsukeRlib {
    /// Build the `netsuke` library with Cargo and locate the resulting rlib.
    ///
    /// Cargo inherits the ambient `RUSTFLAGS` and the pinned toolchain, so the
    /// rlib is borrow-checked exactly as the rest of the suite is. The package
    /// is named `netsuke-build`, but the lib target — and therefore the
    /// `--extern` name — is `netsuke`.
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
                "building the netsuke library failed:\n{}",
                stderr(&output),
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let rlib = stdout
            .lines()
            .filter_map(|line| cargo_artifacts::library_path_in_message(line, "netsuke"))
            .next_back()
            .ok_or_else(|| io::Error::other("cargo reported no netsuke rlib artefact"))?;
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

    /// Type-check `source` with or without the `netsuke` rlib.
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
            args.extend([String::from("--extern"), format!("netsuke={}", self.rlib.display())]);
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
        Command::new(rustc()).arg(response).output()
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

#[path = "support/rustc_response_file.rs"]
mod rustc_response_file;

//! Compile-time tests for public environment-injection APIs.
//!
//! The fixture in `tests/ui/command_env_embedder_pass.rs` imports and
//! constructs `CommandEnv`, `NinjaBuildRequest`, and `NinjaToolRequest`, and
//! references `run_ninja_with`/`run_ninja_tool_with`, exactly as an external
//! embedder would, so a visibility or signature regression fails this suite
//! rather than only the crate's own tests.
//! The cached CLI configuration fixture exercises the equivalent public
//! boundary for `ConfigEnvProvider` and `DiscoveredLayers` through Cargo,
//! which resolves the identical implementation expected by Netsuke.
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
//! scratch project and no toolchain-sensitive `.stderr` snapshot. The `netsuke`
//! rlib is built by Cargo, and the fixture is compiled directly with the
//! workspace `rustc` against it.

#[path = "support/cargo_artifacts.rs"]
mod cargo_artifacts;
