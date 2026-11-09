//! Guard the checks that must reject coverage claimed by prose alone.
//!
//! Each case here was written to fail against a parser that accepted prose as
//! evidence: an identifier named only in a subheading, an introduction, or a
//! reason clause must not satisfy a coverage check. Reverting the corresponding
//! parser rule makes these cases fail, so each one is a regression guard rather
//! than an assertion that happens to hold.

use anyhow::Result;
use rstest::rstest;

use super::fixtures::{
    SIMPLE_ROADMAP, VALID_SKELETON, expect_validation_error,
    inventory_with_register_item_and_unchanged,
};
use super::validation::validate_inventory;

#[test]
fn rejects_an_obligation_traced_only_under_a_deeper_subheading() -> Result<()> {
    let inventory = VALID_SKELETON
        .replace(
            "## Compatibility obligations\n\nPending EP-M3.",
            "## Compatibility obligations\n\n- CO-1: Preserve an obligation.",
        )
        .replace(
            "| -- | -- | -- | -- |\n\n### Consumes",
            concat!(
                "| -- | -- | -- | -- |\n\n",
                "#### Notes\n\n",
                "Task 26.1.1 honours `CO-1`.\n\n",
                "### Consumes",
            ),
        );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "CO-1 has no trace-matrix")?;
    Ok(())
}

#[test]
fn rejects_an_unchanged_entry_under_a_deeper_subheading() -> Result<()> {
    let inventory = inventory_with_register_item_and_unchanged(concat!(
        "#### Notes\n\n",
        "- A1/ast::Rule: appears only under a deeper subheading.\n",
    ));
    expect_validation_error(
        &inventory,
        SIMPLE_ROADMAP,
        "neither traced nor listed unchanged",
    )?;
    Ok(())
}

#[test]
fn rejects_an_exclusion_under_a_deeper_subheading() -> Result<()> {
    let inventory = VALID_SKELETON.replace(
        "### Excluded\n\n- 26.1.2: pending EP-M3.\n",
        concat!(
            "### Excluded\n\n",
            "#### Notes\n\n",
            "- 26.1.2: named only under a deeper subheading.\n",
        ),
    );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "missing: {\"26.1.2\"}")?;
    Ok(())
}

#[test]
fn rejects_a_finding_whose_owner_appears_only_under_a_subheading() -> Result<()> {
    let inventory = VALID_SKELETON.replace(
        "## Findings\n\nPending EP-M3.",
        concat!(
            "## Findings\n\n",
            "- OBS-1: An observed finding has no owner.\n",
            "### Notes\n\n",
            "The task 26.1.1 discusses this finding.\n",
        ),
    );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "owning roadmap task")?;
    Ok(())
}

#[test]
fn rejects_coverage_claimed_only_by_the_trace_introduction() -> Result<()> {
    let inventory = VALID_SKELETON
        .replace(
            "| -- | -- | -- | -- | -- |\n\n### Site rows",
            "| -- | -- | -- | -- | -- |\n| A1/ast::Rule | Rule | -- | -- | -- |\n\n### Site rows",
        )
        .replace(
            "## Trace matrix\n\n",
            "## Trace matrix\n\nA wildcard such as `A1/*` covers the area.\n\n",
        );
    expect_validation_error(
        &inventory,
        SIMPLE_ROADMAP,
        "neither traced nor listed unchanged",
    )?;
    Ok(())
}

#[test]
fn rejects_an_exclusion_bullet_naming_a_task_only_in_its_reason() -> Result<()> {
    let inventory = VALID_SKELETON.replace(
        "- 26.1.2: pending EP-M3.\n",
        "- 26.1.1: pending EP-M3 and blocked-by-26.1.2.\n",
    );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "missing: {\"26.1.2\"}")?;
    Ok(())
}

#[test]
fn rejects_an_obligation_matched_only_by_a_longer_identifier() -> Result<()> {
    let inventory = VALID_SKELETON
        .replace(
            "## Compatibility obligations\n\nPending EP-M3.",
            "## Compatibility obligations\n\n- CO-1: Preserve one.\n- CO-10: Preserve ten.",
        )
        .replace(
            "| -- | -- | -- | -- |\n\n### Excluded",
            "| -- | -- | -- | -- |\n| 26.1.2 | -- | -- | CO-10 |\n\n### Excluded",
        );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "CO-1 has no trace-matrix")?;
    Ok(())
}

#[test]
fn accepts_a_register_item_listed_as_unchanged() -> Result<()> {
    let inventory =
        inventory_with_register_item_and_unchanged("- A1/ast::Rule: no task changes it.\n");
    validate_inventory(&inventory, SIMPLE_ROADMAP)
}

#[rstest]
#[case::no_entry("None yet.\n", "neither traced nor listed unchanged")]
#[case::unnamed_identifier_in_prose(
    "No task changes A1/ast::Rule.\n",
    "neither traced nor listed unchanged"
)]
#[case::malformed_identifier("- A1: no task changes it.\n", "neither traced nor listed unchanged")]
fn rejects_a_register_item_without_a_trace_or_unchanged_entry(
    #[case] unchanged: &str,
    #[case] expected: &str,
) -> Result<()> {
    let inventory = inventory_with_register_item_and_unchanged(unchanged);
    expect_validation_error(&inventory, SIMPLE_ROADMAP, expected)?;
    Ok(())
}
