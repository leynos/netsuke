//! Tests for generating `BuildGraph` from a manifest.
//!
//! Validates the full manifest-to-IR pipeline: that `sources` populate
//! `edge.inputs`, that `deps` populate `edge.implicit_deps` (and are excluded
//! from `edge.inputs`), that recipe command interpolation excludes implicit
//! deps from `$in`/`{{ ins }}`, and that `phony` and error-path cases are
//! handled correctly.  Uses `rstest` parameterisation and reads fixture files
//! from `tests/data/`.

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8PathBuf;
use netsuke::{
    ast::Recipe,
    ir::{BuildGraph, IrGenError},
    manifest, ninja_gen,
};
use rstest::rstest;

#[rstest]
fn minimal_manifest_to_ir() -> Result<()> {
    let manifest = manifest::from_path("tests/data/minimal.yml")?;
    let graph = BuildGraph::from_manifest(&manifest).context("expected graph generation")?;
    ensure!(
        graph.actions.len() == 1,
        "expected one action, got {}",
        graph.actions.len()
    );
    ensure!(
        graph.targets.len() == 1,
        "expected one target, got {}",
        graph.targets.len()
    );
    Ok(())
}

#[rstest]
fn command_list_entries_are_interpolated_in_order() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        rules:
          - name: build
            command:
              - echo first $in
              - echo second $out
        targets:
          - name: out/app
            sources: src/main.c
            rule: build
    "#;
    let manifest = manifest::from_str(yaml)?;
    let graph = BuildGraph::from_manifest(&manifest).context("expected graph generation")?;
    let action = graph
        .actions
        .values()
        .next()
        .context("expected one action")?;
    let Recipe::Command { command } = &action.recipe else {
        bail!("expected a command recipe, got {:?}", action.recipe);
    };
    ensure!(
        command.to_string_vec() == ["echo first src/main.c", "echo second out/app"],
        "each list entry should be interpolated in declaration order: {command:?}"
    );
    Ok(())
}

#[rstest]
fn duplicate_rules_emit_distinct_actions() -> Result<()> {
    let manifest = manifest::from_path("tests/data/duplicate_rules.yml")?;
    let graph = BuildGraph::from_manifest(&manifest).context("expected graph generation")?;
    ensure!(
        graph.actions.len() == 2,
        "expected two actions, got {}",
        graph.actions.len()
    );
    ensure!(
        graph.targets.len() == 2,
        "expected two targets, got {}",
        graph.targets.len()
    );
    Ok(())
}

#[rstest]
fn missing_rule_fails() -> Result<()> {
    let manifest = manifest::from_path("tests/data/missing_rule.yml")?;
    let err = BuildGraph::from_manifest(&manifest)
        .err()
        .context("expected missing rule to produce an error")?;
    ensure!(
        matches!(err, IrGenError::RuleNotFound { .. }),
        "expected missing rule diagnostic, got {err:?}"
    );
    Ok(())
}

#[rstest]
#[case::skipped_target_duplicate_output(
    concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: kept\n",
        "    command: echo kept\n",
        "  - name: kept\n",
        "    command: echo skipped\n",
        "    when: 'false'\n",
    ),
    "kept"
)]
#[case::skipped_action_missing_rule(
    concat!(
        "netsuke_version: '1.0.0'\n",
        "actions:\n",
        "  - name: skipped-action\n",
        "    rule: missing\n",
        "    when: 'false'\n",
        "targets:\n",
        "  - name: kept\n",
        "    command: echo kept\n",
    ),
    "kept"
)]
#[case::skipped_target_cycle(
    concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: kept\n",
        "    command: echo kept\n",
        "  - name: skipped-a\n",
        "    command: echo skipped-a\n",
        "    sources: skipped-b\n",
        "    when: 'false'\n",
        "  - name: skipped-b\n",
        "    command: echo skipped-b\n",
        "    sources: skipped-a\n",
        "    when: 'false'\n",
    ),
    "kept"
)]
fn skipped_manifest_conditions_do_not_contribute_to_ir(
    #[case] yaml: &str,
    #[case] expected_target: &str,
) -> Result<()> {
    let manifest = manifest::from_str(yaml)?;
    let graph = BuildGraph::from_manifest(&manifest).context("expected graph generation")?;
    ensure!(
        graph
            .targets
            .contains_key(&Utf8PathBuf::from(expected_target)),
        "selected target should be present in IR"
    );
    ensure!(
        graph.targets.len() == 1,
        "filtered entries should be absent from IR targets: {:?}",
        graph.targets.keys().collect::<Vec<_>>()
    );
    Ok(())
}

#[rstest]
#[case::target_deps(
    concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: out/app\n",
        "    deps: [include/config.h, generated/stamp]\n",
        "    command: echo $out\n",
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
        "    command: echo $out\n",
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
    let edge = graph
        .targets
        .get(&Utf8PathBuf::from(output))
        .with_context(|| format!("expected edge for {output}"))?;

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

#[rstest]
fn manifest_deps_do_not_contribute_to_recipe_inputs() -> Result<()> {
    let yaml = concat!(
        "netsuke_version: '1.0.0'\n",
        "rules:\n",
        "  - name: compile\n",
        "    command: echo $in {{ ins }} > $out\n",
        "targets:\n",
        "  - name: out/app\n",
        "    sources: src/main.c\n",
        "    deps: [include/config.h, generated/stamp]\n",
        "    rule: compile\n",
    );
    let manifest = manifest::from_str(yaml)?;
    let graph = BuildGraph::from_manifest(&manifest).context("expected graph generation")?;
    let edge = graph
        .targets
        .get(&Utf8PathBuf::from("out/app"))
        .context("expected edge for out/app")?;
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
        .targets
        .iter()
        .flat_map(|(output, edge)| {
            std::iter::once(output)
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

fn assert_conditional_edge(
    graph: &BuildGraph,
    output: &str,
    expected: &ExpectedEdge<'_>,
) -> Result<()> {
    let edge = graph
        .targets
        .get(&Utf8PathBuf::from(output))
        .with_context(|| format!("expected edge for {output}"))?;
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

#[rstest]
fn target_descriptions_do_not_replace_rule_progress_text() -> Result<()> {
    let yaml = concat!(
        "netsuke_version: '1.0.0'\n",
        "rules:\n",
        "  - name: compile\n",
        "    description: Rule progress text\n",
        "    command: echo compile\n",
        "targets:\n",
        "  - name: out/app\n",
        "    description: Target discovery metadata\n",
        "    rule: compile\n",
    );
    let manifest = manifest::from_str(yaml)?;
    let graph = BuildGraph::from_manifest(&manifest).context("generate graph")?;
    let ninja = ninja_gen::generate(&graph).context("generate Ninja manifest")?;

    ensure!(
        ninja.contains("description = Rule progress text"),
        "Ninja progress should use the referenced rule description: {ninja}"
    );
    ensure!(
        !ninja.contains("Target discovery metadata"),
        "target discovery metadata must not appear in Ninja progress: {ninja}"
    );
    Ok(())
}
#[derive(Debug)]
enum ExpectedError {
    DuplicateOutput(Vec<String>),
    MultipleRules {
        target_name: String,
        rules: Vec<String>,
    },
    EmptyRule(String),
    RuleNotFound(String),
    CircularDependency(Vec<String>),
}

#[rstest]
#[case(
    "tests/data/duplicate_outputs.yml",
    ExpectedError::DuplicateOutput(vec!["hello.o".into()])
)]
#[case(
    "tests/data/duplicate_outputs_multi.yml",
    ExpectedError::DuplicateOutput(vec!["bar.o".into(), "foo.o".into()])
)]
#[case(
    "tests/data/multiple_rules_per_target.yml",
    ExpectedError::MultipleRules {
        target_name: "hello.o".into(),
        rules: vec!["compile1".into(), "compile2".into()],
    }
)]
#[case(
    "tests/data/empty_rule.yml",
    ExpectedError::EmptyRule("hello.o".into())
)]
#[case(
    "tests/data/rule_not_found.yml",
    ExpectedError::RuleNotFound("missing_rule".into())
)]
#[case(
    "tests/data/circular.yml",
    ExpectedError::CircularDependency(vec!["a".into(), "b".into(), "a".into()])
)]
fn manifest_error_cases(
    #[case] manifest_path: &str,
    #[case] expected: ExpectedError,
) -> Result<()> {
    let manifest = manifest::from_path(manifest_path)?;
    let err = BuildGraph::from_manifest(&manifest)
        .err()
        .with_context(|| format!("expected {manifest_path} to produce an error"))?;
    match (err, expected) {
        (
            IrGenError::DuplicateOutput { outputs, .. },
            ExpectedError::DuplicateOutput(exp_outputs),
        ) => {
            ensure!(
                outputs == exp_outputs,
                "unexpected duplicate outputs: got {outputs:?}, expected {exp_outputs:?}"
            );
        }
        (
            IrGenError::MultipleRules {
                target_name, rules, ..
            },
            ExpectedError::MultipleRules {
                target_name: exp_target,
                rules: exp_rules,
            },
        ) => {
            ensure!(
                target_name == exp_target,
                "unexpected target: got {target_name}, expected {exp_target}"
            );
            ensure!(
                rules == exp_rules,
                "unexpected rules: got {rules:?}, expected {exp_rules:?}"
            );
        }
        (IrGenError::EmptyRule { target_name, .. }, ExpectedError::EmptyRule(exp_target)) => {
            ensure!(
                target_name == exp_target,
                "unexpected target: got {target_name}, expected {exp_target}"
            );
        }
        (IrGenError::RuleNotFound { rule_name, .. }, ExpectedError::RuleNotFound(exp_rule)) => {
            ensure!(
                rule_name == exp_rule,
                "unexpected rule: got {rule_name}, expected {exp_rule}"
            );
        }
        (
            IrGenError::CircularDependency {
                cycle,
                missing_dependencies,
                ..
            },
            ExpectedError::CircularDependency(exp_cycle),
        ) => {
            ensure!(
                missing_dependencies.is_empty(),
                "missing dependencies should be empty in manifest fixtures"
            );
            let mut expected_cycle: Vec<Utf8PathBuf> =
                exp_cycle.iter().map(Utf8PathBuf::from).collect();
            let mut actual = cycle;
            expected_cycle.sort();
            actual.sort();
            ensure!(
                actual == expected_cycle,
                "unexpected dependency cycle: got {actual:?}, expected {expected_cycle:?}"
            );
        }
        (other, exp) => bail!("expected {exp:?} but got {other:?}"),
    }
    Ok(())
}

#[path = "ir_from_manifest_tests/dependency_order.rs"]
mod dependency_order;
