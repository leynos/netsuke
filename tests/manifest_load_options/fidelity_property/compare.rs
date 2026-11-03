//! The three-layer comparison one generated manifest is put through.
//!
//! Split from `fidelity_property/mod.rs`, which owns the property and its
//! coverage floor, to keep both files under Whitaker's raw-line module cap. Each
//! layer is compared separately so a failure names which one drifted: the
//! canonical manifest, the [`GraphView`] projection of the lowered graph, or the
//! emitted Ninja text.
//!
//! [`GraphView`]: netsuke::graph_view::GraphView

use anyhow::Result;
use netsuke::{
    ast::NetsukeManifest,
    graph_view::GraphView,
    ir::BuildGraph,
    manifest::{
        EnvAccessPolicy, ManifestEnvironment, ManifestLoadOptions, ManifestName, TestSurface,
        from_str_with_options,
    },
    ninja_gen::generate_with_shell,
    recipe_shell::RecipeShell,
    stdlib::StdlibConfig,
};
use proptest::test_runner::TestCaseError;

use super::super::fixtures::one_var_reader;
use super::spec::Spec;

/// Load `yaml` through the build registration with a fixed clock.
fn load_full(yaml: &str, config: &StdlibConfig) -> Result<NetsukeManifest> {
    let reader = one_var_reader();
    let environment = ManifestEnvironment::new(&reader, EnvAccessPolicy::default());
    from_str_with_options(
        yaml,
        &ManifestName::new("Netsukefile"),
        ManifestLoadOptions::full(config.clone(), environment),
    )
}

/// Load `yaml` through the test registration with the same fixed clock.
fn load_test(yaml: &str, config: &StdlibConfig) -> Result<NetsukeManifest> {
    let surface = TestSurface::new(config.clone()).with_env_reader(one_var_reader());
    from_str_with_options(
        yaml,
        &ManifestName::new("Netsukefile"),
        ManifestLoadOptions::test(surface),
    )
}

/// Fail a property case with a message naming the spec that produced it.
fn fail(spec: &Spec, message: impl std::fmt::Display) -> TestCaseError {
    TestCaseError::fail(format!("{message}\n--- manifest ---\n{}", spec.yaml()))
}

/// Compare the manifest, graph projection, and Ninja text of one spec.
pub(super) fn compare_modes(
    spec: &Spec,
    config: &StdlibConfig,
) -> std::result::Result<(), TestCaseError> {
    let yaml = spec.yaml();
    let full = load_full(&yaml, config)
        .map_err(|error| fail(spec, format_args!("Full load failed: {error:#}")))?;
    let test = load_test(&yaml, config)
        .map_err(|error| fail(spec, format_args!("Test load failed: {error:#}")))?;

    let full_json = serde_json_canonicalizer::to_string(&full).map_err(|error| {
        fail(
            spec,
            format_args!("Full manifest did not serialize: {error}"),
        )
    })?;
    let test_json = serde_json_canonicalizer::to_string(&test).map_err(|error| {
        fail(
            spec,
            format_args!("Test manifest did not serialize: {error}"),
        )
    })?;
    if full_json != test_json {
        return Err(fail(
            spec,
            format_args!("manifests differ:\nFull: {full_json}\nTest: {test_json}"),
        ));
    }

    let full_graph = BuildGraph::from_manifest_for_shell(&full, RecipeShell::Posix)
        .map_err(|error| fail(spec, format_args!("Full graph failed: {error}")))?;
    let test_graph = BuildGraph::from_manifest_for_shell(&test, RecipeShell::Posix)
        .map_err(|error| fail(spec, format_args!("Test graph failed: {error}")))?;
    if GraphView::from_build_graph(&full_graph) != GraphView::from_build_graph(&test_graph) {
        return Err(fail(spec, "graph projections differ"));
    }

    let full_ninja = generate_with_shell(&full_graph, RecipeShell::Posix)
        .map_err(|error| fail(spec, format_args!("Full Ninja generation failed: {error}")))?;
    let test_ninja = generate_with_shell(&test_graph, RecipeShell::Posix)
        .map_err(|error| fail(spec, format_args!("Test Ninja generation failed: {error}")))?;
    if full_ninja != test_ninja {
        return Err(fail(
            spec,
            format_args!("Ninja text differs:\nFull:\n{full_ninja}\nTest:\n{test_ninja}"),
        ));
    }
    Ok(())
}
