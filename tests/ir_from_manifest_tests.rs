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
    manifest,
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
        command == "echo src/main.c src/main.c > out/app",
        "deps should not appear in recipe interpolation: {command}"
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
                "unexpected duplicate outputs: got {:?}, expected {:?}",
                outputs,
                exp_outputs
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
                "unexpected rules: got {:?}, expected {:?}",
                rules,
                exp_rules
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
                "unexpected dependency cycle: got {:?}, expected {:?}",
                actual,
                expected_cycle
            );
        }
        (other, exp) => bail!("expected {exp:?} but got {other:?}"),
    }
    Ok(())
}
