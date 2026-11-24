//! Limit-specific cases: empty iterators, oversized items, and telemetry.

use super::exhaustion_kind;
use crate::manifest::{
    ManifestBudgetLimits, ManifestValue, budget::ManifestBudget, expand_foreach_with_budget,
    from_str_with_limits,
};
use anyhow::{Context, Result, ensure};
use metrics_util::{
    MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use minijinja::Environment;
use rstest::rstest;

#[test]
fn empty_foreach_succeeds_when_the_source_alone_exceeds_the_allowance() -> Result<()> {
    let env = Environment::new();
    let yaml = format!(
        concat!(
            "targets:\n",
            "  - name: never\n",
            "    foreach: []\n",
            "    command: 'echo {}'\n",
        ),
        "x".repeat(5_000)
    );
    let limits = ManifestBudgetLimits {
        source_bytes: 16_384,
        expanded_bytes: 512,
        ..ManifestBudgetLimits::default()
    };
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;
    let budget = ManifestBudget::new(limits)?;

    expand_foreach_with_budget(&mut doc, &env, &budget, true)
        .context("an empty iterator must clone nothing and therefore fit")?;
    let targets = doc
        .get("targets")
        .and_then(ManifestValue::as_array)
        .context("the fixture must retain its targets array")?;
    ensure!(
        targets.is_empty(),
        "an empty foreach must expand to no entries, got {}",
        targets.len()
    );
    Ok(())
}

#[test]
fn oversized_filtered_item_is_rejected_before_condition_evaluation() -> Result<()> {
    let env = Environment::new();
    let oversized = "x".repeat(4_096);
    let yaml = format!(
        concat!(
            "targets:\n",
            "  - name: '{{{{ item }}}}'\n",
            "    foreach: ['{oversized}']\n",
            "    when: 'missing_helper()'\n",
            "    command: echo ok\n",
        ),
        oversized = oversized
    );
    let limits = ManifestBudgetLimits {
        expanded_bytes: 512,
        source_bytes: 16_384,
        ..ManifestBudgetLimits::default()
    };
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;
    let budget = ManifestBudget::new(limits)?;

    let error = expand_foreach_with_budget(&mut doc, &env, &budget, true)
        .expect_err("the oversized item must be refused before `when` is evaluated");
    ensure!(
        exhaustion_kind(&error) == Some(crate::manifest::budget::ManifestBudgetKind::ExpandedBytes),
        "an oversized item must fail on retained bytes before the undefined helper: {error:#}"
    );
    Ok(())
}

#[rstest]
fn expansion_byte_exhaustion_records_expanded_bytes_telemetry() {
    let yaml = format!(
        concat!(
            "netsuke_version: 1.0.0\n",
            "targets:\n",
            "  - name: '{{{{ item }}}}'\n",
            "    foreach: [one, two]\n",
            "    command: '{}'\n",
        ),
        "x".repeat(5_000)
    );
    let limits = ManifestBudgetLimits {
        source_bytes: 16_384,
        expanded_bytes: 4_096,
        ..ManifestBudgetLimits::default()
    };
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let error = metrics::with_local_recorder(&recorder, || {
        from_str_with_limits(&yaml, limits)
            .expect_err("the cloned expansion must exceed the byte budget")
    });
    let snapshot = snapshotter.snapshot().into_vec();
    let counters = snapshot
        .iter()
        .filter_map(|(key, _, _, value)| {
            (key.kind() == MetricKind::Counter
                && key.key().name() == "netsuke_manifest_budget_exhausted_total")
                .then_some((key.key().labels().collect::<Vec<_>>(), value))
        })
        .collect::<Vec<_>>();
    assert_eq!(counters.len(), 1, "expansion exhaustion should count once");
    let (labels, value) = counters
        .first()
        .expect("the asserted budget counter must supply labels and a value");
    assert_eq!(labels.len(), 2, "budget telemetry must use two labels");
    assert!(
        labels
            .iter()
            .any(|label| label.key() == "stage" && label.value() == "expansion_aggregate"),
        "unexpected telemetry stage: {labels:?}"
    );
    assert!(
        labels
            .iter()
            .any(|label| label.key() == "budget" && label.value() == "expanded_bytes"),
        "unexpected telemetry budget: {labels:?}"
    );
    assert!(
        matches!(value, DebugValue::Counter(1)),
        "one expansion exhaustion must increment the counter once"
    );
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("resource budget exhausted"),
        "the error must retain the localized budget diagnostic: {rendered}"
    );
    assert!(
        !rendered.contains("xxxxxxxxxx"),
        "the diagnostic must not include the oversized recipe text"
    );
}
