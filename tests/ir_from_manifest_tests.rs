//! Tests for generating `BuildGraph` from a manifest.

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
fn duplicate_rules_are_deduped() {
    let manifest = manifest::from_path("tests/data/duplicate_rules.yml").expect("load");
    let graph = BuildGraph::from_manifest(&manifest).expect("ir");
    assert_eq!(graph.actions.len(), 1);
    assert_eq!(graph.targets.len(), 2);
}

#[rstest]
fn missing_rule_fails() {
    let manifest = manifest::from_path("tests/data/missing_rule.yml").expect("load");
    let err = BuildGraph::from_manifest(&manifest).expect_err("error");
    assert!(matches!(err, IrGenError::RuleNotFound { .. }));
}

#[rstest]
fn duplicate_outputs_fail() {
    let manifest = manifest::from_path("tests/data/duplicate_outputs.yml").expect("load");
    let err = BuildGraph::from_manifest(&manifest).expect_err("error");
    match err {
        IrGenError::DuplicateOutput { outputs } => {
            assert_eq!(outputs, vec![String::from("hello.o")]);
        }
        _ => panic!("wrong error"),
    }
}

#[rstest]
fn multiple_rules_per_target_fails() {
    let manifest = manifest::from_path("tests/data/multiple_rules_per_target.yml").expect("load");
    let err = BuildGraph::from_manifest(&manifest).expect_err("error");
    match err {
        IrGenError::MultipleRules { target_name, rules } => {
            assert_eq!(target_name, "hello.o");
            assert_eq!(rules, vec![String::from("compile1"), String::from("compile2")]);
        }
        _ => panic!("wrong error"),
    }
}

#[rstest]
fn duplicate_outputs_multi_listed() {
    let manifest = manifest::from_path("tests/data/duplicate_outputs_multi.yml").expect("load");
    let err = BuildGraph::from_manifest(&manifest).expect_err("error");
    match err {
        IrGenError::DuplicateOutput { outputs } => {
            assert_eq!(outputs, vec![String::from("bar.o"), String::from("foo.o")]);
        }
        _ => panic!("wrong error"),
    }
}
