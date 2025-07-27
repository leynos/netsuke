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

#[test]
fn missing_rule_fails() {
    let manifest = manifest::from_path("tests/data/missing_rule.yml").expect("load");
    let err = BuildGraph::from_manifest(&manifest).expect_err("error");
    matches!(err, IrGenError::RuleNotFound { .. });
}

#[test]
fn duplicate_outputs_fail() {
    let manifest = manifest::from_path("tests/data/duplicate_outputs.yml").expect("load");
    let err = BuildGraph::from_manifest(&manifest).expect_err("error");
    matches!(err, IrGenError::DuplicateOutput { .. });
}
