//! Tests for generating `BuildGraph` from a manifest.

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8PathBuf;
use netsuke::{
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
        (IrGenError::DuplicateOutput { outputs }, ExpectedError::DuplicateOutput(exp_outputs)) => {
            ensure!(
                outputs == exp_outputs,
                "unexpected duplicate outputs: got {:?}, expected {:?}",
                outputs,
                exp_outputs
            );
        }
        (
            IrGenError::MultipleRules { target_name, rules },
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
        (IrGenError::EmptyRule { target_name }, ExpectedError::EmptyRule(exp_target)) => {
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
