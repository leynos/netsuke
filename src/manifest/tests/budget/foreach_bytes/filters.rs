//! Filtered-item and shared-allowance cases for the expansion-byte budget.

use super::{exhaustion_kind, expand_fixture, minimal_admitted_bytes, target_map};
use crate::manifest::{ManifestValue, budget::retained_size};
use anyhow::{Context, Result, ensure};
use minijinja::{Environment, value::Value};

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

    // Establish the two charges independently: a filtered expansion needs the
    // source and item estimates but no retained-clone charge, while every
    // retained item pays all three. A non-zero charge for a filtered clone
    // would move the filtered minimum off this exact value.
    let source_bytes = retained_size::source_map_bytes(&target_map(filtered)?, usize::MAX)
        .context("the filtered source must fit an unbounded ceiling")?;
    let item_bytes = retained_size::item_value_bytes(
        &Value::from_serialize(ManifestValue::String("a".into())),
        usize::MAX,
    )
    .context("a short filtered item must fit an unbounded ceiling")?;
    ensure!(
        filtered_minimum == source_bytes.max(item_bytes),
        "filtered expansion must charge only the source and item estimates: minimum={filtered_minimum}, source={source_bytes}, item={item_bytes}"
    );
    let retained_charge = source_bytes
        .checked_add(item_bytes)
        .and_then(|total| total.checked_add(retained_size::iteration_overhead()))
        .context("the retained-entry charge must not overflow")?;
    let expected_retained = retained_charge
        .checked_mul(3)
        .context("three retained-entry charges must not overflow")?;
    ensure!(
        retained_minimum == expected_retained,
        "each retained item must charge the source, item, and injected keys: minimum={retained_minimum}, expected={expected_retained}"
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
    let action_only = concat!(
        "actions:\n",
        "  - name: 'act-{{ item }}'\n",
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
    let action_minimum = minimal_admitted_bytes(action_only, &env)?;
    let combined_minimum = minimal_admitted_bytes(combined, &env)?;
    let expected_combined = target_minimum
        .checked_add(action_minimum)
        .context("the two section charges must not overflow")?;
    ensure!(
        combined_minimum == expected_combined,
        "one shared allowance must sum both sections: combined={combined_minimum}, target={target_minimum}, action={action_minimum}"
    );

    // A ceiling that admits either section alone must still refuse their
    // aggregate. An implementation that resets the allowance between sections
    // would succeed here.
    let joint = target_minimum.max(action_minimum);
    expand_fixture(target_only, &env, joint)
        .context("the target section must fit the joint allowance")?;
    expand_fixture(action_only, &env, joint)
        .context("the action section must fit the joint allowance")?;
    let error = expand_fixture(combined, &env, joint)
        .expect_err("the aggregate must exceed the joint allowance");
    ensure!(
        exhaustion_kind(&error) == Some(crate::manifest::budget::ManifestBudgetKind::ExpandedBytes),
        "unexpected failure kind for the shared allowance: {error:#}"
    );
    Ok(())
}
