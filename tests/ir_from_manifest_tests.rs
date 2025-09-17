//! Tests for generating `BuildGraph` from a manifest.

use camino::Utf8PathBuf;
use netsuke::{
    ir::{BuildGraph, IrGenError},
    manifest,
};
use rstest::rstest;

#[rstest]
fn minimal_manifest_to_ir() {
    let manifest = manifest::from_path("tests/data/minimal.yml").expect("load");
    let graph = BuildGraph::from_manifest(&manifest).expect("ir");
    assert_eq!(graph.actions.len(), 1);
    assert_eq!(graph.targets.len(), 1);
}

#[rstest]
fn duplicate_rules_emit_distinct_actions() {
    let manifest = manifest::from_path("tests/data/duplicate_rules.yml").expect("load");
    let graph = BuildGraph::from_manifest(&manifest).expect("ir");
    assert_eq!(graph.actions.len(), 2);
    assert_eq!(graph.targets.len(), 2);
}

#[rstest]
fn missing_rule_fails() {
    let manifest = manifest::from_path("tests/data/missing_rule.yml").expect("load");
    let err = BuildGraph::from_manifest(&manifest).expect_err("error");
    assert!(matches!(err, IrGenError::RuleNotFound { .. }));
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
fn manifest_error_cases(#[case] manifest_path: &str, #[case] expected: ExpectedError) {
    let manifest = manifest::from_path(manifest_path).expect("load");
    let err = BuildGraph::from_manifest(&manifest).expect_err("error");
    match (err, expected) {
        (IrGenError::DuplicateOutput { outputs }, ExpectedError::DuplicateOutput(exp_outputs)) => {
            assert_eq!(outputs, exp_outputs);
        }
        (
            IrGenError::MultipleRules { target_name, rules },
            ExpectedError::MultipleRules {
                target_name: exp_target,
                rules: exp_rules,
            },
        ) => {
            assert_eq!(target_name, exp_target);
            assert_eq!(rules, exp_rules);
        }
        (IrGenError::EmptyRule { target_name }, ExpectedError::EmptyRule(exp_target)) => {
            assert_eq!(target_name, exp_target);
        }
        (IrGenError::RuleNotFound { rule_name, .. }, ExpectedError::RuleNotFound(exp_rule)) => {
            assert_eq!(rule_name, exp_rule);
        }
        (
            IrGenError::CircularDependency {
                cycle,
                missing_dependencies,
            },
            ExpectedError::CircularDependency(exp_cycle),
        ) => {
            assert!(
                missing_dependencies.is_empty(),
                "missing dependencies should be empty in manifest fixtures"
            );
            let mut expected_cycle: Vec<Utf8PathBuf> =
                exp_cycle.iter().map(Utf8PathBuf::from).collect();
            let mut actual = cycle;
            expected_cycle.sort();
            actual.sort();
            assert_eq!(actual, expected_cycle);
        }
        (other, exp) => panic!("expected {exp:?} but got {other:?}"),
    }
}
