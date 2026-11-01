//! Step definitions for options-carrying manifest loading scenarios.
//!
//! The scenarios observe the two surfaces the way a manifest author meets
//! them. The first loads one pure manifest twice — once through
//! `ManifestLoadOptions::full` and once through `ManifestLoadOptions::test` —
//! and requires the emitted Ninja text to agree, so a capability difference
//! that changed the meaning of a pure manifest would fail here rather than in
//! the differential suites. The second loads a manifest that pipes a command
//! through `shell`, which the build surface runs literally and the test
//! surface refuses; the refusal must reach the author as an error naming both
//! the refusal class and the helper.
//!
//! Fixtures are read from the repository rather than staged into a temporary
//! workspace: the loader entry point takes manifest text, so the scenario
//! needs a source file and nothing else.

use anyhow::{Context as _, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use netsuke::{
    ast::NetsukeManifest,
    ir::BuildGraph,
    manifest::{self, ManifestLoadOptions, ManifestName, TestSurface, from_str_with_options},
    ninja_gen::generate_with_shell,
    recipe_shell::RecipeShell,
    stdlib::StdlibConfig,
};
use rstest_bdd_macros::{given, then, when};
use test_support::display_error_chain;

use super::manifest::environment::manifest_env_reader;
use super::stdlib::parse_iso_timestamp;
use crate::bdd::fixtures::TestWorld;
use crate::bdd::helpers::parse_store::store_parse_outcome;

/// Return the repository root, resolved at compile time.
fn repo_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Return the manifest path the scenario selected.
fn selected_path(world: &TestWorld) -> Result<Utf8PathBuf> {
    world
        .load_options_path
        .get()
        .map(Utf8PathBuf::from)
        .context("the scenario must select a manifest file first")
}

/// Read one repository-relative fixture.
fn read_fixture(relative: &Utf8Path) -> Result<String> {
    ensure!(
        relative.is_relative(),
        "a fixture path must be repository-relative: {relative}"
    );
    let root = Dir::open_ambient_dir(repo_root(), ambient_authority())
        .context("open the repository root")?;
    root.read_to_string(relative.as_str())
        .with_context(|| format!("read `{relative}`"))
}

/// Return the stdlib configuration, carrying the scenario's clock when set.
///
/// An unset clock is left at the default so a scenario that does not fix a
/// clock observes the same ambient behaviour a caller would.
fn scenario_config(world: &TestWorld) -> Result<StdlibConfig> {
    let config = StdlibConfig::from_current_dir()?;
    Ok(match world.stdlib_clock.get() {
        Some(clock) => config.with_clock(clock),
        None => config,
    })
}

/// Load `text` through the build registration.
fn load_build(
    world: &TestWorld,
    text: &str,
    name: &ManifestName,
    config: StdlibConfig,
) -> Result<NetsukeManifest> {
    let reader = manifest_env_reader(world);
    let policy = world.manifest_env_access_policy.borrow().clone();
    let environment = manifest::ManifestEnvironment::new(&reader, policy);
    from_str_with_options(text, name, ManifestLoadOptions::full(config, environment))
        .context("the build surface must load the scenario manifest")
}

/// Render a loaded manifest to Ninja text.
fn ninja_text(manifest: &NetsukeManifest) -> Result<String> {
    let graph = BuildGraph::from_manifest_for_shell(manifest, RecipeShell::Posix)
        .context("lower the loaded manifest")?;
    generate_with_shell(&graph, RecipeShell::Posix).context("generate Ninja text")
}

/// Select the manifest the scenario loads.
#[given("the manifest file {path:string}")]
fn select_manifest(world: &TestWorld, path: &str) -> Result<()> {
    ensure!(
        !path.trim().is_empty(),
        "manifest path must not be an empty string"
    );
    world.load_options_path.set(path.to_owned());
    Ok(())
}

/// Fix the clock both surfaces are configured with.
#[given("a fixed clock at {instant:string}")]
fn fix_clock(world: &TestWorld, instant: &str) -> Result<()> {
    let parsed = parse_iso_timestamp(instant)?;
    world.stdlib_clock.set(netsuke::stdlib::fixed_clock(parsed));
    Ok(())
}

/// Load the selected manifest through both registrations.
#[when("the manifest is loaded in build mode and in test mode")]
fn load_in_both_modes(world: &TestWorld) -> Result<()> {
    let path = selected_path(world)?;
    let text = read_fixture(&path)?;
    let config = scenario_config(world)?;
    let name = ManifestName::new(path.as_str());

    let build = load_build(world, &text, &name, config.clone())?;
    let test = from_str_with_options(
        &text,
        &name,
        ManifestLoadOptions::test(TestSurface::new(config)),
    )
    .context("the test surface must load a pure manifest")?;

    world.load_options_build_ninja.set(ninja_text(&build)?);
    world.load_options_test_ninja.set(ninja_text(&test)?);
    Ok(())
}

/// Load the selected manifest through the test registration only.
#[when("the manifest is loaded in test mode")]
fn load_in_test_mode(world: &TestWorld) -> Result<()> {
    let path = selected_path(world)?;
    let text = read_fixture(&path)?;
    let config = scenario_config(world)?;
    let outcome = from_str_with_options(
        &text,
        &ManifestName::new(path.as_str()),
        ManifestLoadOptions::test(TestSurface::new(config)),
    )
    .map_err(|error| display_error_chain(error.as_ref()));
    store_parse_outcome(&world.manifest, &world.manifest_error, outcome);
    Ok(())
}

/// Assert both registrations emitted the same Ninja text.
#[then("both loads produce the same Ninja file")]
fn both_loads_agree(world: &TestWorld) -> Result<()> {
    let build = world
        .load_options_build_ninja
        .get()
        .context("the build load must have run")?;
    let test = world
        .load_options_test_ninja
        .get()
        .context("the test load must have run")?;
    ensure!(
        build == test,
        "the surfaces emitted different Ninja text.\nBuild:\n{build}\nTest:\n{test}"
    );
    Ok(())
}
