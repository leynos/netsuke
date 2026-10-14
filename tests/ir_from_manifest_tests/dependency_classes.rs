//! Tests for manifest dependency classes in lowered IR edges.

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8PathBuf;
use netsuke::{ast::Recipe, ir::BuildGraph, manifest, recipe_shell::RecipeShell};
use rstest::rstest;

/// Resolve the canonical edge that produces `output`.
fn edge_for_output<'a>(graph: &'a BuildGraph, output: &str) -> Result<&'a netsuke::ir::BuildEdge> {
    graph
        .target_for_output(Utf8PathBuf::from(output).as_path())
        .map(|(_, edge)| edge)
        .with_context(|| format!("expected edge for {output}"))
}

/// Lower manifest dependencies into the implicit Ninja dependency class.
#[rstest]
#[case::target_deps(
    concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: out/app\n",
        "    deps: [include/config.h, generated/stamp]\n",
        "    command: echo {{ outs }}\n",
    ),
    "out/app",
    false,
)]
#[case::action_deps(
    concat!(
        "netsuke_version: '1.0.0'\n",
        "actions:\n",
        "  - name: regenerate\n",
        "    deps: [schemas/user.yml, tools/generator]\n",
        "    command: echo {{ outs }}\n",
        "targets: []\n",
    ),
    "regenerate",
    true,
)]
fn manifest_deps_populate_implicit_deps(
    #[case] yaml: &str,
    #[case] output: &str,
    #[case] expected_phony: bool,
) -> Result<()> {
    let manifest = manifest::from_str(yaml)?;
    let graph = BuildGraph::from_manifest(&manifest).context("expected graph generation")?;
    let edge = edge_for_output(&graph, output)?;

    ensure!(
        edge.implicit_deps
            == vec![
                Utf8PathBuf::from(if expected_phony {
                    "schemas/user.yml"
                } else {
                    "include/config.h"
                }),
                Utf8PathBuf::from(if expected_phony {
                    "tools/generator"
                } else {
                    "generated/stamp"
                }),
            ],
        "unexpected implicit deps for {output}: {:?}",
        edge.implicit_deps
    );
    ensure!(
        edge.inputs.is_empty(),
        "deps must not be explicit recipe inputs: {:?}",
        edge.inputs
    );
    ensure!(
        edge.phony == expected_phony,
        "unexpected phony flag for {output}: {}",
        edge.phony
    );
    Ok(())
}

/// Exclude manifest dependencies from recipe input interpolation.
#[rstest]
fn manifest_deps_do_not_contribute_to_recipe_inputs() -> Result<()> {
    let yaml = concat!(
        "netsuke_version: '1.0.0'\n",
        "rules:\n",
        "  - name: compile\n",
        "    command: echo {{ ins }} {{ ins }} > {{ outs }}\n",
        "targets:\n",
        "  - name: out/app\n",
        "    sources: src/main.c\n",
        "    deps: [include/config.h, generated/stamp]\n",
        "    rule: compile\n",
    );
    let manifest = manifest::from_str(yaml)?;
    let graph = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::Posix)
        .context("expected graph generation")?;
    let edge = edge_for_output(&graph, "out/app")?;
    let action = graph
        .actions
        .get(&edge.action_id)
        .context("expected action for out/app")?;
    let Recipe::Command { command } = &action.recipe else {
        bail!("expected command recipe");
    };

    ensure!(
        command.as_single() == Some("echo src/main.c src/main.c > out/app"),
        "deps should not appear in recipe interpolation: {command:?}"
    );
    ensure!(
        edge.inputs == vec![Utf8PathBuf::from("src/main.c")],
        "sources should remain the explicit inputs"
    );
    ensure!(
        edge.implicit_deps
            == vec![
                Utf8PathBuf::from("include/config.h"),
                Utf8PathBuf::from("generated/stamp"),
            ],
        "deps should populate only implicit deps"
    );
    Ok(())
}

/// Preserve distinct explicit, implicit, and order-only dependency classes.
#[rstest]
fn conditional_action_deps_populate_distinct_ir_classes() -> Result<()> {
    let manifest = manifest::from_path("tests/data/conditional_action_deps.yml")?;
    let graph = BuildGraph::from_manifest(&manifest).context("expected graph generation")?;

    assert_conditional_edge(
        &graph,
        "fallback-alpha",
        &ExpectedEdge {
            inputs: &["src/alpha.in"],
            implicit_deps: &["build/alpha.o", "shared/action.cfg"],
            order_only_deps: &["order/alpha.stamp"],
            is_phony: true,
        },
    )?;
    assert_conditional_edge(
        &graph,
        "fallback-beta",
        &ExpectedEdge {
            inputs: &["src/beta.in"],
            implicit_deps: &["build/beta.o", "shared/action.cfg"],
            order_only_deps: &["order/beta.stamp"],
            is_phony: true,
        },
    )?;
    assert_conditional_edge(
        &graph,
        "out/fallback",
        &ExpectedEdge {
            inputs: &["src/target.in"],
            implicit_deps: &["include/fallback.h"],
            order_only_deps: &["order/target.stamp"],
            is_phony: false,
        },
    )?;

    let rendered_paths = graph
        .edges()
        .flat_map(|edge| {
            edge.explicit_outputs
                .iter()
                .chain(&edge.inputs)
                .chain(&edge.implicit_deps)
                .chain(&edge.order_only_deps)
        })
        .map(|path| path.as_str())
        .collect::<Vec<_>>();
    ensure!(
        rendered_paths
            .iter()
            .all(|path| !path.starts_with("preferred")),
        "filtered branches should not contribute paths to the IR: {rendered_paths:?}"
    );
    Ok(())
}

struct ExpectedEdge<'a> {
    inputs: &'a [&'a str],
    implicit_deps: &'a [&'a str],
    order_only_deps: &'a [&'a str],
    is_phony: bool,
}

/// Assert one conditional action's IR edge and dependency classes.
fn assert_conditional_edge(
    graph: &BuildGraph,
    output: &str,
    expected: &ExpectedEdge<'_>,
) -> Result<()> {
    let edge = edge_for_output(graph, output)?;
    let expected_paths = |paths: &[&str]| {
        paths
            .iter()
            .copied()
            .map(Utf8PathBuf::from)
            .collect::<Vec<_>>()
    };
    ensure!(
        edge.inputs == expected_paths(expected.inputs),
        "unexpected explicit inputs for {output}: {:?}",
        edge.inputs
    );
    ensure!(
        edge.implicit_deps == expected_paths(expected.implicit_deps),
        "unexpected implicit deps for {output}: {:?}",
        edge.implicit_deps
    );
    ensure!(
        edge.order_only_deps == expected_paths(expected.order_only_deps),
        "unexpected order-only deps for {output}: {:?}",
        edge.order_only_deps
    );
    ensure!(
        edge.phony == expected.is_phony,
        "unexpected phony flag for {output}: {}",
        edge.phony
    );
    Ok(())
}
