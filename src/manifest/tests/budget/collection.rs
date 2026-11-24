//! Verify collection consumption is bounded through manifest loading.
//!
//! The `compact` filter charges a crate-internal allowance that the public
//! limit type deliberately does not expose. These cases construct small
//! ceilings directly so they can assert the exact stage and limit without
//! pulling millions of members, and so a regression cannot hide behind a
//! default that is merely large.

use crate::manifest::budget::ManifestBudget;
use crate::manifest::{
    ManifestBudgetLimits, from_path_for_manifest_query_with_budget, from_str_with_budget,
};
use anyhow::{Context, Result, ensure};
use metrics_util::{
    MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use tempfile::tempdir;
use test_support::fluent::normalize_fluent_isolates;

/// Build manifest accounting with small crate-internal collection ceilings.
fn budget_with(collection_call_items: usize, collection_items: usize) -> Result<ManifestBudget> {
    ManifestBudget::with_collection_limits(
        ManifestBudgetLimits::default(),
        collection_call_items,
        collection_items,
    )
}

/// Render the diagnostic exactly as a user sees it.
fn rendered(error: &anyhow::Error) -> String {
    normalize_fluent_isolates(&format!("{error:#}"))
}

/// Copy one metric key's labels so they outlive the snapshot entry.
fn owned_labels(key: &metrics::Key) -> Vec<(String, String)> {
    key.labels()
        .map(|label| (label.key().to_owned(), label.value().to_owned()))
        .collect()
}

/// Report whether a copied label set contains the named pair.
fn has_label(labels: &[(String, String)], key: &str, value: &str) -> bool {
    labels
        .iter()
        .any(|(label_key, label_value)| label_key == key && label_value == value)
}

#[test]
fn call_ceiling_rejects_a_huge_lazy_repetition_quickly() -> Result<()> {
    let yaml = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: huge\n",
        "    command: \"{{ ([''] * 1000000000000) | compact | length }}\"\n",
    );
    let error = from_str_with_budget(yaml, budget_with(4, 8)?)
        .expect_err("a trillion-member repetition must stop at the call ceiling");
    let message = rendered(&error);
    ensure!(
        message.contains("resource budget exhausted"),
        "the failure must report the resource limit: {message}"
    );
    ensure!(
        message.contains("during collection"),
        "the failure must name the collection stage: {message}"
    );
    ensure!(
        message.contains('4'),
        "the failure must report the configured call limit: {message}"
    );
    ensure!(
        !message.contains("1000000000000"),
        "the diagnostic must not echo manifest data: {message}"
    );
    Ok(())
}

#[test]
fn aggregate_ceiling_spans_two_compact_calls() -> Result<()> {
    let yaml = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: first\n",
        "    command: \"{{ ([''] * 4) | compact | length }}\"\n",
        "  - name: second\n",
        "    command: \"{{ ([''] * 4) | compact | length }}\"\n",
    );
    let error = from_str_with_budget(yaml, budget_with(4, 6)?)
        .expect_err("the second four-member call must exhaust the six-member aggregate");
    let message = rendered(&error);
    ensure!(
        message.contains("during collection_aggregate"),
        "the failure must name the aggregate stage: {message}"
    );
    ensure!(
        message.contains('6'),
        "the failure must report the configured aggregate limit: {message}"
    );
    Ok(())
}

#[test]
fn a_separate_load_restores_the_collection_aggregate() -> Result<()> {
    let yaml = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: first\n",
        "    command: \"{{ ([''] * 4) | compact | length }}\"\n",
    );
    let first = from_str_with_budget(yaml, budget_with(4, 4)?)
        .context("the exact aggregate must load in its own manifest")?;
    ensure!(
        first.targets.len() == 1,
        "the fixture must load its single target"
    );

    let second = from_str_with_budget(yaml, budget_with(4, 4)?)
        .context("a fresh budget must restore the aggregate for a second load")?;
    ensure!(
        second.targets.len() == 1,
        "the second manifest must load its single target"
    );
    Ok(())
}

#[test]
fn full_load_records_collection_exhaustion_telemetry() -> Result<()> {
    let yaml = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: bounded\n",
        "    command: \"{{ ([''] * 5) | compact | length }}\"\n",
    );
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let budget = budget_with(4, 8)?;
    let error = metrics::with_local_recorder(&recorder, move || {
        from_str_with_budget(yaml, budget)
            .expect_err("five members must exceed the four-member call ceiling")
    });
    ensure!(rendered(&error).contains("during collection"));

    let counters = snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .filter_map(|(key, _, _, value)| {
            (key.kind() == MetricKind::Counter
                && key.key().name() == "netsuke_manifest_budget_exhausted_total")
                .then_some((owned_labels(key.key()), value))
        })
        .collect::<Vec<_>>();
    ensure!(
        counters.len() == 1,
        "the collection exhaustion must be recorded exactly once"
    );
    let (labels, value) = counters
        .first()
        .expect("the asserted counter must carry labels and a value");
    ensure!(
        has_label(labels, "stage", "collection"),
        "the counter must carry the collection stage: {labels:?}"
    );
    ensure!(
        has_label(labels, "budget", "collection_call_items"),
        "the counter must carry the call-width budget label: {labels:?}"
    );
    ensure!(
        matches!(value, DebugValue::Counter(1)),
        "the counter value must be one: {value:?}"
    );
    Ok(())
}

#[test]
fn query_load_enforces_the_ceiling_without_budget_telemetry() -> Result<()> {
    let workspace = tempdir().context("create collection query workspace")?;
    let path = workspace.path().join("Netsukefile");
    test_support::fs::write(
        &path,
        concat!(
            "netsuke_version: 1.0.0\n",
            "targets:\n",
            "  - name: query\n",
            "    description: \"{{ ([''] * 5) | compact | length }}\"\n",
            "    command: echo ok\n",
        ),
    )?;

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let budget = budget_with(4, 8)?;
    let error = metrics::with_local_recorder(&recorder, move || {
        from_path_for_manifest_query_with_budget(&path, budget, None)
            .expect_err("query rendering must enforce the same collection ceiling")
    });
    ensure!(
        rendered(&error).contains("during collection"),
        "the query failure must name the collection stage: {error:#}"
    );
    ensure!(
        snapshotter
            .snapshot()
            .into_vec()
            .iter()
            .all(|(key, _, _, _)| {
                key.key().name() != "netsuke_manifest_budget_exhausted_total"
            }),
        "a query failure must not emit build-shaped budget telemetry"
    );
    Ok(())
}

#[test]
fn telemetry_uses_only_the_closed_collection_labels() -> Result<()> {
    let yaml = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: first\n",
        "    command: \"{{ ([''] * 4) | compact | length }}\"\n",
        "  - name: second\n",
        "    command: \"{{ ([''] * 4) | compact | length }}\"\n",
    );
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let budget = budget_with(4, 6)?;
    let error = metrics::with_local_recorder(&recorder, move || {
        from_str_with_budget(yaml, budget).expect_err("the second call must exhaust the aggregate")
    });
    ensure!(rendered(&error).contains("during collection_aggregate"));

    let labels = snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .filter_map(|(key, _, _, _)| {
            (key.key().name() == "netsuke_manifest_budget_exhausted_total")
                .then_some(owned_labels(key.key()))
        })
        .collect::<Vec<_>>();
    ensure!(
        labels.len() == 1,
        "the aggregate exhaustion must be recorded exactly once"
    );
    let labels = labels.first().expect("the counter must carry labels");
    ensure!(
        has_label(labels, "stage", "collection_aggregate"),
        "the counter must carry the aggregate stage: {labels:?}"
    );
    ensure!(
        has_label(labels, "budget", "collection_items"),
        "the counter must carry the aggregate budget label: {labels:?}"
    );
    Ok(())
}
