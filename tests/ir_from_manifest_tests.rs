//! Tests for generating `BuildGraph` from a manifest.
//!
//! Validates the full manifest-to-IR pipeline: that `sources` populate
//! `edge.inputs`, that `deps` populate `edge.implicit_deps` (and are excluded
//! from `edge.inputs`), that recipe command interpolation excludes implicit
//! deps from `{{ ins }}`, and that `phony` and error-path cases are
//! handled correctly.  Uses `rstest` parameterisation and reads fixture files
//! from `tests/data/`.

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8PathBuf;
use netsuke::{
    ast::{Recipe, StringOrList},
    ir::{BuildGraph, IrGenError},
    manifest, ninja_gen,
    recipe_shell::RecipeShell,
};
use rstest::rstest;

#[path = "ir_from_manifest_tests/canonical_edge_storage.rs"]
mod canonical_edge_storage;
#[path = "ir_from_manifest_tests/dependency_classes.rs"]
mod dependency_classes;

/// Generate the expected action and target counts for each manifest fixture.
#[rstest]
#[case::minimal_manifest("tests/data/minimal.yml", 1, 1)]
#[case::duplicate_rules("tests/data/duplicate_rules.yml", 2, 2)]
fn manifest_fixture_generates_expected_ir(
    #[case] manifest_path: &str,
    #[case] expected_actions: usize,
    #[case] expected_targets: usize,
) -> Result<()> {
    let manifest = manifest::from_path(manifest_path)?;
    let graph = BuildGraph::from_manifest(&manifest).context("expected graph generation")?;
    ensure!(
        graph.actions.len() == expected_actions,
        "expected {expected_actions} actions, got {}",
        graph.actions.len(),
    );
    ensure!(
        graph.output_count() == expected_targets,
        "expected {expected_targets} targets, got {}",
        graph.output_count()
    );
    Ok(())
}

/// Preserve declaration order while interpolating command-list entries.
#[rstest]
fn command_list_entries_are_interpolated_in_order() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        rules:
          - name: build
            command:
              - echo first {{ ins }}
              - echo second {{ outs }}
        targets:
          - name: out/app
            sources: src/main.c
            rule: build
    "#;
    let manifest = manifest::from_str(yaml)?;
    let graph = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::Posix)
        .context("expected graph generation")?;
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

/// Report an IR error when a target names no declared rule.
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

/// Exclude skipped conditional entries from all IR validation and graph output.
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
            .target_for_output(Utf8PathBuf::from(expected_target).as_path())
            .is_some(),
        "selected target should be present in IR"
    );
    ensure!(
        graph.output_count() == 1,
        "filtered entries should be absent from IR targets: {:?}",
        graph.output_paths().collect::<Vec<_>>()
    );
    Ok(())
}

/// Lower dependency-only entries into deduplicated phony actions.
#[test]
fn dependency_only_entries_lower_to_deduplicated_phony_actions() -> Result<()> {
    let yaml = concat!(
        "netsuke_version: '1.0.0'\n",
        "actions:\n",
        "  - name: all\n",
        "    deps: [check-fmt, lint]\n",
        "targets:\n",
        "  - name: release\n",
        "    deps: [all]\n",
    );
    let manifest = manifest::from_str(yaml)?;
    let graph = BuildGraph::from_manifest(&manifest).context("expected graph generation")?;
    ensure!(
        graph.actions.len() == 1
            && graph.actions.values().all(|action| {
                matches!(
                    action.recipe,
                    Recipe::Command {
                        command: StringOrList::Empty
                    }
                )
            }),
        "dependency-only entries should deduplicate into one non-executable action: {:?}",
        graph.actions
    );
    let ninja = ninja_gen::generate(&graph).context("generate dependency-only Ninja")?;
    ensure!(
        ninja.contains("build all: phony | check-fmt lint")
            && ninja.contains("build release: phony | all"),
        "dependency-only entries should lower to phony Ninja nodes: {ninja}"
    );
    ensure!(
        !ninja.contains("rule ") && !ninja.contains("command ="),
        "dependency-only nodes must not emit a synthetic recipe: {ninja}"
    );
    Ok(())
}

/// Keep a target description separate from its rule's progress text.
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

/// Map invalid manifest structures to their expected IR generation errors.
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
