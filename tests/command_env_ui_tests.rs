//! Compile-time tests for the explicit Ninja environment API.
//!
//! The fixture in `tests/ui/command_env_embedder_pass.rs` imports and
//! constructs `CommandEnv`, `NinjaBuildRequest`, and `NinjaToolRequest`, and
//! references `run_ninja_with`/`run_ninja_tool_with`, exactly as an external
//! embedder would, so a visibility or signature regression fails this suite
//! rather than only the crate's own tests.
//!
//! There is deliberately no compile-fail case for the removed APIs
//! (`EnvMut`, `PathGuard`, `prepend_dir_to_path`, `override_ninja_env`): the
//! workspace build already rejects any revived call site, and pinning rustc's
//! diagnostic wording for a missing item would make the suite fail on
//! compiler upgrades without guarding anything extra.
//!
//! Trybuild cannot drive this: it removes ambient `RUSTFLAGS` and overrides
//! workspace `build.rustflags` outright, so it would rebuild the `netsuke`
//! dependency without `-Zpolonius=next` and reject the crate's `POLONIUS(...)`
//! sites (see docs/polonius.md). Instead the `netsuke` rlib is built by Cargo
//! — which does inherit the ambient flags — and the fixture is compiled
//! directly with the workspace `rustc` against that rlib.

use std::{
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

/// Compile one external public-API fixture through the direct-rustc harness.
fn compile_public_api_fixture(source: &str, failure_message: &str) -> io::Result<()> {
    let rlib = NetsukeRlib::build()?;
    let output = rlib.compile(source)?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{failure_message}:\n{}",
            stderr(&output)
        )));
    }
    Ok(())
}

/// The `netsuke` rlib and the deps directory holding its dependencies.
struct NetsukeRlib {
    rlib: PathBuf,
    deps_dir: PathBuf,
}

impl NetsukeRlib {
    /// Build the `netsuke` library with Cargo and locate the resulting rlib.
    ///
    /// Cargo inherits the ambient `RUSTFLAGS`, so the rlib is borrow-checked
    /// with the same Polonius flags as the rest of the suite. The package is
    /// named `netsuke-build`, but the lib target — and therefore the
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
            .filter_map(netsuke_rlib_in_message)
            .next_back()
            .ok_or_else(|| io::Error::other("cargo reported no netsuke rlib artefact"))?;
        // Cargo uplifts the top-level package's artefacts out of `deps/` into
        // the profile directory, so the rlib's own parent is not necessarily
        // where the dependency rlibs live.
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
    /// `--emit=metadata` is enough to surface any visibility or signature
    /// regression while sparing the harness a full link of `netsuke`'s
    /// dependency tree.
    fn compile(&self, source: &str) -> io::Result<Output> {
        let output_dir = tempfile::tempdir()?;
        Command::new(rustc())
            .arg("--edition=2024")
            .arg("--crate-type=bin")
            .arg("--emit=metadata")
            .arg(manifest_dir().join(source))
            .arg("--extern")
            .arg(format!("netsuke={}", self.rlib.display()))
            .arg("-L")
            .arg(format!("dependency={}", self.deps_dir.display()))
            .arg("-o")
            .arg(output_dir.path().join("command-env-ui.rmeta"))
            .output()
    }
}

/// Extract the `netsuke` lib rlib path from one Cargo JSON message, if any.
///
/// The package also ships a `netsuke` bin target; requiring an `.rlib`
/// filename keeps the filter on the library artefact.
fn netsuke_rlib_in_message(line: &str) -> Option<PathBuf> {
    let message: serde_json::Value = serde_json::from_str(line).ok()?;
    if message.get("reason")? != "compiler-artifact"
        || message.get("target")?.get("name")? != "netsuke"
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
        .next()
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
