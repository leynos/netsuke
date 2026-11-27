//! Regression cases for the aggregate expansion-byte allowance.
//!
//! `foreach` expansion clones one complete entry per admitted item. These
//! cases pin the allowance that bounds those retained clones and prove that a
//! refused item stops lazy iteration instead of materializing the whole
//! sequence. Filtered and limit-specific cases live in child modules.

mod filters;
mod limits;

use crate::manifest::{
    ManifestBudgetLimits, ManifestMap, ManifestValue, budget::ManifestBudget,
    expand_foreach_with_budget, lazy_sequence::CountingSequence,
};
use anyhow::{Context, Result, ensure};
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

/// Return the first target mapping in a fixture, for estimator comparisons.
fn target_map(yaml: &str) -> Result<ManifestMap> {
    let doc: ManifestValue = serde_saphyr::from_str(yaml)?;
    doc.get("targets")
        .and_then(ManifestValue::as_array)
        .and_then(|targets| targets.first())
        .and_then(ManifestValue::as_object)
        .cloned()
        .context("fixture must contain one target mapping")
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
