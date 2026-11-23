//! Rule-selector cardinality contracts in manifest-to-IR lowering.
//!
//! A `rule` field names exactly one rule, as a scalar or a one-element list.
//! These cases drive the public manifest and `BuildGraph` entry points, so the
//! rejection shape, its bounded diagnostic and the supported selector forms are
//! all asserted where a caller can observe them.

use anyhow::{Context, Result, ensure};
use netsuke::{
    ir::{BuildGraph, IrGenError},
    manifest,
};
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;
use test_support::{localizer_test_lock, set_en_localizer};

/// Names the diagnostic is documented to report before summarising the rest.
///
/// Stated here rather than imported from the implementation, so the test pins
/// the published contract rather than agreeing with whatever the cap becomes.
const REPORTED_PREFIX_LEN: usize = 8;

/// Build a manifest whose single target selects `rules` in the given order.
fn manifest_selecting(rules: &[&str]) -> String {
    let mut source = String::from(
        "netsuke_version: '1.0.0'\nrules:\n  - name: build\n    command: echo built\ntargets:\n  - name: out.txt\n    rule:\n",
    );
    for name in rules {
        source.push_str("      - ");
        source.push_str(name);
        source.push('\n');
    }
    source
}

/// Lower `source` and return the graph or the IR error it produced.
fn lower(source: &str) -> Result<std::result::Result<BuildGraph, IrGenError>> {
    let manifest = manifest::from_str(source).context("the manifest should parse")?;
    Ok(BuildGraph::from_manifest(&manifest))
}

/// Report the target, rule names and rendered message a multi-name selector
/// produced.
///
/// Fluent isolates every placeable with direction marks, which would sit
/// between a numeral and its noun; the message is normalised so assertions can
/// read it as prose.
fn multiple_rules(source: &str) -> Result<(String, Vec<String>, String)> {
    let error = lower(source)?
        .err()
        .context("a multi-name selector must fail IR generation")?;
    let IrGenError::MultipleRules {
        target_name,
        rules,
        message,
    } = error
    else {
        anyhow::bail!("expected MultipleRules, got {error:?}");
    };
    Ok((
        target_name,
        rules,
        normalize_fluent_isolates(&message.to_string()),
    ))
}

/// Report every supplied name in ascending order, duplicates included.
#[rstest]
#[case::unsorted_with_duplicate(&["c", "a", "b", "a"], &["a", "a", "b", "c"])]
#[case::identical_pair(&["a", "a"], &["a", "a"])]
#[case::already_sorted(&["a", "b"], &["a", "b"])]
fn reportable_selector_names_are_sorted_and_keep_duplicates(
    #[case] supplied: &[&str],
    #[case] expected: &[&str],
) -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _localizer = set_en_localizer();
    let (target_name, rules, message) = multiple_rules(&manifest_selecting(supplied))?;

    ensure!(
        target_name == "out.txt",
        "expected target out.txt, got {target_name}"
    );
    let expected: Vec<String> = expected.iter().map(|name| (*name).to_owned()).collect();
    ensure!(
        rules == expected,
        "expected sorted names {expected:?}, got {rules:?}"
    );
    ensure!(
        !message.contains("more names"),
        "a reportable selector must not claim omitted names: {message}"
    );
    Ok(())
}

/// A one-element list is a supported scalar-equivalent selector.
#[test]
fn a_single_element_list_selects_its_rule() -> Result<()> {
    let graph = lower(&manifest_selecting(&["build"]))?
        .map_err(|error| anyhow::anyhow!("[build] should be a valid selector: {error:?}"))?;
    ensure!(
        graph.output_count() == 1,
        "expected one output from the selected rule, got {}",
        graph.output_count()
    );
    Ok(())
}

/// A one-element list naming an undeclared rule is still a lookup failure.
#[test]
fn a_single_element_list_reports_an_unknown_rule() -> Result<()> {
    let error = lower(&manifest_selecting(&["missing"]))?
        .err()
        .context("an unknown one-element selector must fail IR generation")?;
    let IrGenError::RuleNotFound { rule_name, .. } = error else {
        anyhow::bail!("expected RuleNotFound, got {error:?}");
    };
    ensure!(
        rule_name == "missing",
        "expected the unknown rule name, got {rule_name}"
    );
    Ok(())
}

/// A generated long selector reports only its bounded sorted prefix.
///
/// Four thousand names make the difference between a quadratic and a bounded
/// diagnostic unmistakable, and the manifest stays far below the default
/// source-byte and expanded-entry ceilings, so the case exercises lowering
/// rather than the loader's budgets.
#[rstest]
fn a_generated_long_selector_reports_only_the_smallest_names() -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _localizer = set_en_localizer();
    const SUPPLIED: usize = 4096;
    let supplied: Vec<String> = (0..SUPPLIED)
        .rev()
        .map(|index| format!("name_{index:04}"))
        .collect();
    let borrowed: Vec<&str> = supplied.iter().map(String::as_str).collect();

    let (_, rules, message) = multiple_rules(&manifest_selecting(&borrowed))?;

    let expected: Vec<String> = (0..REPORTED_PREFIX_LEN)
        .map(|index| format!("name_{index:04}"))
        .collect();
    ensure!(
        rules == expected,
        "expected the eight lexically smallest names {expected:?}, got {rules:?}"
    );
    let omitted = SUPPLIED - REPORTED_PREFIX_LEN;
    ensure!(
        message.contains(&format!("{omitted} more names")),
        "the message must report {omitted} omitted names: {message}"
    );
    Ok(())
}
