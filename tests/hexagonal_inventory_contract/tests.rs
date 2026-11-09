//! Exercise the inventory integrity checks against the shared fixtures.

use anyhow::Result;
use rstest::rstest;

use super::fixtures::{
    PIN, SIMPLE_ROADMAP, TRANSITIVE_ROADMAP, VALID_SKELETON, expect_validation_error,
    transitive_skeleton,
};
use super::validation::validate_inventory;

#[test]
fn valid_skeleton_covers_every_dependent_task_as_pending() -> Result<()> {
    validate_inventory(VALID_SKELETON, SIMPLE_ROADMAP)
}

#[test]
fn area_and_site_wildcards_cover_register_identifiers() -> Result<()> {
    let inventory = VALID_SKELETON
        .replace(
            "| -- | -- | -- | -- | -- |\n\n### Site rows",
            "| -- | -- | -- | -- | -- |\n| A1/ast::Rule | Rule | -- | -- | -- |\n\n### Site rows",
        )
        .replace(
            "| -- | -- | -- | -- | -- | -- | -- |\n\n## Landed work",
            "| -- | -- | -- | -- | -- | -- | -- |\n| A1/ast::Rule@Recipe::new#constructor | -- | constructor | -- | -- | -- | -- |\n\n## Landed work",
        )
        .replace(
            "| -- | -- | -- | -- |\n\n### Consumes",
            "| 26.1.2 | A1/* A1/ast::Rule@* | -- | -- |\n\n### Consumes",
        );
    validate_inventory(&inventory, SIMPLE_ROADMAP)
}

#[rstest]
#[case::area_only("| A1 | Rule | -- | -- | -- |", "is malformed")]
#[case::unknown_area("| A8/ast::Rule | Rule | -- | -- | -- |", "is malformed")]
#[case::unsupported_site_kind(
    "| A1/ast::Rule@Recipe::new#builder | Rule | -- | -- | -- |",
    "is malformed"
)]
#[case::duplicate_identifier(
    "| A1/ast::Rule | Rule | -- | -- | -- |\n| A1/ast::Rule | Rule | -- | -- | -- |",
    "should be unique"
)]
fn rejects_malformed_or_duplicate_register_rows(
    #[case] rows: &str,
    #[case] expected_error: &str,
) -> Result<()> {
    let inventory = VALID_SKELETON.replace(
        "| -- | -- | -- | -- | -- |\n\n### Site rows",
        &format!("| -- | -- | -- | -- | -- |\n{rows}\n\n### Site rows"),
    );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, expected_error)?;
    Ok(())
}

#[rstest]
#[case::second_distinct_hash(
    &format!("Pin: {PIN}\nabcdef0123456789abcdef0123456789abcdef01\n\n## Scope and definitions"),
    "only its declared"
)]
#[case::no_pin_line("## Scope and definitions", "exactly one `Pin:` line")]
#[case::duplicate_identical_lines(
    &format!("Pin: {PIN}\nPin: {PIN}\n\n## Scope and definitions"),
    "exactly one `Pin:` line"
)]
fn rejects_inventories_without_exactly_one_valid_pin(
    #[case] replacement: &str,
    #[case] expected: &str,
) -> Result<()> {
    let inventory = VALID_SKELETON.replace(
        &format!("Pin: {PIN}\n\n## Scope and definitions"),
        replacement,
    );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, expected)?;
    Ok(())
}

#[test]
fn rejects_a_trace_row_for_an_unknown_roadmap_task() -> Result<()> {
    let inventory = VALID_SKELETON.replace(
        "| Task | Register items | Findings | Obligations |\n| -- | -- | -- | -- |\n\n### Consumes",
        "| Task | Register items | Findings | Obligations |\n| -- | -- | -- | -- |\n| 29.9.9 | -- | -- | -- |\n\n### Consumes",
    );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "absent from the roadmap")?;
    Ok(())
}

#[test]
fn rejects_a_missing_transitive_dependent_even_if_named_elsewhere() -> Result<()> {
    let inventory = transitive_skeleton()
        .replace("- 26.1.3: pending EP-M3.\n", "")
        .replace(
            "## Scope and definitions",
            "## Purpose\n26.1.3 appears only outside the trace matrix.\n\n## Scope and definitions",
        );
    expect_validation_error(&inventory, TRANSITIVE_ROADMAP, "missing: {\"26.1.3\"}")?;
    Ok(())
}

#[test]
fn rejects_an_obligation_without_a_trace_reference() -> Result<()> {
    let inventory = VALID_SKELETON.replace(
        "## Compatibility obligations\n\nPending EP-M3.",
        "## Compatibility obligations\n\n- CO-1: Preserve an obligation.",
    );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "CO-1 has no trace-matrix")?;
    Ok(())
}

#[test]
fn rejects_an_obligation_claimed_only_by_the_trace_introduction() -> Result<()> {
    let inventory = VALID_SKELETON
        .replace(
            "## Compatibility obligations\n\nPending EP-M3.",
            "## Compatibility obligations\n\n- CO-1: Preserve an obligation.",
        )
        .replace(
            "## Trace matrix\n\n",
            "## Trace matrix\n\nEvery task honours `CO-1`.\n\n",
        );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "CO-1 has no trace-matrix")?;
    Ok(())
}

#[test]
fn rejects_a_finding_without_an_owner() -> Result<()> {
    let inventory = VALID_SKELETON.replace(
        "## Findings\n\nPending EP-M3.",
        "## Findings\n\n- OBS-1: An observed finding has no owner.",
    );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "owning roadmap task")?;
    Ok(())
}
