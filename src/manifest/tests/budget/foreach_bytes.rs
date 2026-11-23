//! Regression cases for the aggregate expansion-byte allowance.
//!
//! `foreach` expansion clones one complete entry per admitted item. These
//! cases pin the allowance that bounds those retained clones, prove that
//! filtered items do not consume it, and prove that a refused item stops lazy
//! iteration instead of materializing the whole sequence.

use crate::manifest::{
    ManifestBudgetLimits, ManifestValue, budget::ManifestBudget, expand_foreach_with_budget,
    from_str_with_limits, lazy_sequence::CountingSequence,
};
use anyhow::{Context, Result, ensure};
use metrics_util::{
    MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use minijinja::Environment;
use rstest::rstest;

/// Largest limit the boundary search may try before declaring the fixture
/// unbounded.
const SEARCH_CEILING: usize = 1 << 24;

/// Return the typed budget exhaustion carried by an expansion failure.
fn exhaustion_kind(error: &anyhow::Error) -> Option<crate::manifest::budget::ManifestBudgetKind> {
    error
        .chain()
        .find_map(|source| {
            source.downcast_ref::<crate::manifest::budget::ManifestBudgetExhaustion>()
        })
        .map(|exhaustion| exhaustion.kind)
}

/// Expand one fixture under the supplied byte ceiling.
fn expand_fixture(
    yaml: &str,
    env: &Environment<'static>,
    expanded_bytes: usize,
) -> Result<ManifestValue> {
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;
    let limits = ManifestBudgetLimits {
        expanded_bytes,
        ..ManifestBudgetLimits::default()
    };
    let budget = ManifestBudget::new(limits)?;
    expand_foreach_with_budget(&mut doc, env, &budget, true)?;
    Ok(doc)
}

/// Find the smallest expansion-byte ceiling that admits the whole fixture.
///
/// The allowance is monotonic: a ceiling that succeeds also succeeds when it
/// grows, so a binary search over the refusal boundary yields the exact
/// retained-byte charge without coupling the test to the estimator's model.
fn minimal_admitted_bytes(yaml: &str, env: &Environment<'static>) -> Result<usize> {
    let mut refused = 0usize;
    let mut admitted = 1usize;
    while expand_fixture(yaml, env, admitted).is_err() {
        refused = admitted;
        admitted = admitted
            .checked_mul(2)
            .filter(|candidate| *candidate <= SEARCH_CEILING)
            .context("fixture never fits the expansion-byte search ceiling")?;
    }
    while admitted.saturating_sub(refused) > 1 {
        // A shift keeps the search clear of the workspace's denied integer
        // division; the distance is a byte count, and halving it is exact
        // enough to converge.
        let midpoint = refused + ((admitted - refused) >> 1);
        if expand_fixture(yaml, env, midpoint).is_ok() {
            admitted = midpoint;
        } else {
            refused = midpoint;
        }
    }
    Ok(admitted)
}

/// Build an environment whose `counting_range()` yields at most `total` values.
fn counting_environment(sequence: &CountingSequence) -> Environment<'static> {
    let mut env = Environment::new();
    crate::manifest::lazy_sequence::add_counting_range(&mut env, sequence);
    env
}

/// A single-item fixture repeated inline keeps the per-item charge constant.
fn inline_fixture(item_len: usize, item_count: usize) -> String {
    let item = "x".repeat(item_len);
    let items = std::iter::repeat_n(format!("'{item}'"), item_count)
        .collect::<Vec<_>>()
        .join(", ");
    format!("targets:\n  - name: '{{{{ item }}}}'\n    foreach: [{items}]\n    command: echo ok\n")
}

#[rstest]
#[case::one_item(1)]
#[case::two_items(2)]
fn exact_expansion_bytes_boundary_is_admitted_and_one_less_is_refused(
    #[case] item_count: usize,
) -> Result<()> {
    let env = Environment::new();
    let yaml = inline_fixture(256, item_count);
    let admitted = minimal_admitted_bytes(&yaml, &env)?;

    expand_fixture(&yaml, &env, admitted).context("the exact boundary must be admitted")?;

    let error = expand_fixture(&yaml, &env, admitted - 1)
        .expect_err("one byte below the boundary must be refused");
    ensure!(
        exhaustion_kind(&error) == Some(crate::manifest::budget::ManifestBudgetKind::ExpandedBytes),
        "unexpected failure kind for the boundary: {error:#}"
    );
    Ok(())
}

#[test]
fn refused_item_stops_lazy_iteration_after_the_admitted_item() -> Result<()> {
    const TOTAL: usize = 100_000;
    // A large recipe entry is the input the issue reported: the allowed
    // iteration count multiplies this mapping once per produced value.
    let yaml = format!(
        concat!(
            "targets:\n",
            "  - name: '{{{{ item }}}}'\n",
            "    foreach: 'counting_range()'\n",
            "    command: 'echo {}'\n",
        ),
        "x".repeat(512)
    );

    // The two-value sequence measures the exact charge for two retained
    // entries. One byte less admits a single entry and refuses the second,
    // which a hundred-thousand-value sequence must reach without consuming the
    // rest of the iterator.
    let measured = CountingSequence::new(2);
    let two_item_charge = minimal_admitted_bytes(&yaml, &counting_environment(&measured))?;

    let sequence = CountingSequence::new(TOTAL);
    let produced = sequence.produced();
    let error = expand_fixture(&yaml, &counting_environment(&sequence), two_item_charge - 1)
        .expect_err("the second produced item must be refused");
    ensure!(
        exhaustion_kind(&error) == Some(crate::manifest::budget::ManifestBudgetKind::ExpandedBytes),
        "unexpected failure kind for the lazy refusal: {error:#}"
    );
    ensure!(
        produced.load(std::sync::atomic::Ordering::SeqCst) == 2,
        "expansion must stop after the refused item instead of consuming {TOTAL} values"
    );
    Ok(())
}

#[test]
fn filtered_foreach_items_do_not_consume_expansion_bytes() -> Result<()> {
    let env = Environment::new();
    let filtered = concat!(
        "targets:\n",
        "  - name: '{{ item }}'\n",
        "    foreach: [a, b, c]\n",
        "    when: 'false'\n",
        "    command: echo ok\n",
    );
    let retained = concat!(
        "targets:\n",
        "  - name: '{{ item }}'\n",
        "    foreach: [a, b, c]\n",
        "    when: 'true'\n",
        "    command: echo ok\n",
    );
    let filtered_minimum = minimal_admitted_bytes(filtered, &env)?;
    let retained_minimum = minimal_admitted_bytes(retained, &env)?;
    ensure!(
        filtered_minimum < retained_minimum,
        "filtered items must retain no cloned entry: filtered={filtered_minimum}, retained={retained_minimum}"
    );

    expand_fixture(filtered, &env, filtered_minimum)
        .context("filtered expansion must fit within the source and item estimates")?;
    let error = expand_fixture(retained, &env, filtered_minimum)
        .expect_err("retained entries must need more than the filtered allowance");
    ensure!(
        exhaustion_kind(&error) == Some(crate::manifest::budget::ManifestBudgetKind::ExpandedBytes),
        "unexpected failure kind for retained entries below their charge: {error:#}"
    );
    Ok(())
}

#[test]
fn expansion_bytes_are_shared_across_targets_and_actions() -> Result<()> {
    let env = Environment::new();
    let target_only = concat!(
        "targets:\n",
        "  - name: '{{ item }}'\n",
        "    foreach: [one, two, three]\n",
        "    command: echo ok\n",
    );
    let combined = concat!(
        "targets:\n",
        "  - name: '{{ item }}'\n",
        "    foreach: [one, two, three]\n",
        "    command: echo ok\n",
        "actions:\n",
        "  - name: 'act-{{ item }}'\n",
        "    foreach: [one, two, three]\n",
        "    command: echo ok\n",
    );
    let target_minimum = minimal_admitted_bytes(target_only, &env)?;
    let combined_minimum = minimal_admitted_bytes(combined, &env)?;
    ensure!(
        combined_minimum > target_minimum,
        "the actions section must draw from the same allowance: target={target_minimum}, combined={combined_minimum}"
    );

    let error = expand_fixture(combined, &env, target_minimum)
        .expect_err("both sections must not fit one section's allowance");
    ensure!(
        exhaustion_kind(&error) == Some(crate::manifest::budget::ManifestBudgetKind::ExpandedBytes),
        "unexpected failure kind for the shared allowance: {error:#}"
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
