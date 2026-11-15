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
    SIMPLE_ROADMAP, VALID_SKELETON, expect_validation_error, inventory_with_one_register_item,
    inventory_with_register_item_and_unchanged,
};
use super::validation::validate_inventory;

/// Removing a required section must fail, not leave its checks unvisited.
///
/// `heading_body` returning `None` is what a missing heading looks like, and
/// several checks read a section through `unwrap_or_default`. Without
/// `validate_required_sections` those checks would read an empty string and
/// report success, so each required heading earns its own failing case.
#[rstest]
#[case::scope(
    "## Scope and definitions\n\nDefinitions pending.\n\n",
    "Scope and definitions"
)]
#[case::method("## Method\n\nMethod pending.\n\n", "Method")]
#[case::register("## Register\n\n", "Register")]
#[case::landed_work("## Landed work\n\nPending EP-M3.\n\n", "Landed work")]
#[case::compatibility_obligations(
    "## Compatibility obligations\n\nPending EP-M3.\n\n",
    "Compatibility obligations"
)]
#[case::refreshing(
    "## Refreshing this inventory\n\nPending.\n",
    "Refreshing this inventory"
)]
fn rejects_an_inventory_missing_a_required_heading(
    #[case] removed: &str,
    #[case] expected_heading: &str,
) -> Result<()> {
    let inventory = VALID_SKELETON.replace(removed, "");
    expect_validation_error(
        &inventory,
        SIMPLE_ROADMAP,
        &format!("should contain the `## {expected_heading}` section"),
    )
}

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

/// A finding may name a task that exists but is still the wrong owner.
///
/// `validate_findings` checks two different things: that a finding names an
/// owner at all, and that each named task appears in the pinned roadmap. Only
/// the first is covered by the ownerless case, so an owner naming a plausible
/// but absent task would otherwise pass.
#[test]
fn rejects_a_finding_owned_by_a_task_absent_from_the_roadmap() -> Result<()> {
    let inventory = VALID_SKELETON.replace(
        "## Findings\n\nPending EP-M3.",
        "## Findings\n\n- H1: Owned by 29.9.9.\n",
    );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "finding owner 29.9.9")?;
    Ok(())
}

/// An exclusion must carry a reason, not merely name its task.
///
/// `exclusion_identifier` accepts `- 26.1.2: reason`, so a bullet ending at the
/// colon still parses as an exclusion. Coverage then looks satisfied while the
/// reader is told nothing about why the task is out of scope.
#[test]
fn rejects_an_exclusion_that_gives_no_reason() -> Result<()> {
    let inventory = VALID_SKELETON.replace("- 26.1.2: pending EP-M3.\n", "- 26.1.2:\n");
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "should give a reason")?;
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

/// A register item counts as traced only when a real task row names it.
///
/// Every case is a claim that must not satisfy coverage, and each one satisfies
/// part of the rule while breaking the rest. The prose cases name the
/// identifier and nothing else; a sentence ending in a full stop would not
/// prove them, because `reference_covers` tokenises on whitespace and the stray
/// punctuation leaves the token unequal to the identifier, so the rule would
/// look live while only the punctuation was doing the work. The row cases are
/// the pipe-delimited shapes: the first is a well-formed wildcard on a line
/// that belongs to no `Task`-headed table, the second is a real task in a row
/// of the skeleton's own `Task` table whose reference cell is a wildcard with
/// no area name, and the third is such a row whose first cell names no task at
/// all. Accepting any of them would let something that traces nothing cover a
/// register item.
#[rstest]
#[case::prose_under_changes("### Changes\n\n", "### Changes\n\nA1/ast::Rule\n\n")]
#[case::prose_under_the_trace_introduction(
    "## Trace matrix\n\n",
    "## Trace matrix\n\nA wildcard such as `A1/*` covers the area.\n\n"
)]
#[case::row_outside_a_task_table(
    "### Changes\n\n",
    "### Changes\n\n| not-a-task | A1/* | -- | -- |\n\n"
)]
#[case::area_wildcard_with_no_area(
    "| -- | -- | -- | -- |\n\n### Consumes",
    "| -- | -- | -- | -- |\n| 26.1.2 | /* | -- | -- |\n\n### Consumes"
)]
#[case::task_row_naming_no_task(
    "| -- | -- | -- | -- |\n\n### Consumes",
    "| -- | -- | -- | -- |\n| nobody | A1/* | -- | -- |\n\n### Consumes"
)]
fn rejects_register_coverage_claimed_by_something_other_than_a_task_row(
    #[case] anchor: &str,
    #[case] replacement: &str,
) -> Result<()> {
    let inventory = inventory_with_one_register_item().replace(anchor, replacement);
    expect_validation_error(
        &inventory,
        SIMPLE_ROADMAP,
        "neither traced nor listed unchanged",
    )?;
    Ok(())
}

#[test]
fn rejects_an_obligation_claimed_only_by_prose_under_consumes() -> Result<()> {
    let inventory = VALID_SKELETON
        .replace(
            "## Compatibility obligations\n\nPending EP-M3.",
            "## Compatibility obligations\n\n- CO-1: Preserve an obligation.",
        )
        .replace("### Consumes\n\n", "### Consumes\n\n`CO-1`\n\n");
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "CO-1 has no trace-matrix")?;
    Ok(())
}

#[test]
fn rejects_task_coverage_claimed_by_a_table_under_a_deeper_excluded_heading() -> Result<()> {
    let inventory = VALID_SKELETON.replace(
        "### Excluded\n\n- 26.1.2: pending EP-M3.\n",
        concat!(
            "### Excluded\n\n",
            "#### Notes\n\n",
            "| Task | Register items | Findings | Obligations |\n",
            "| -- | -- | -- | -- |\n",
            "| 26.1.2 | -- | -- | -- |\n",
        ),
    );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "missing: {\"26.1.2\"}")?;
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

/// Renaming one register table's header must not exempt its rows.
///
/// The renamed table still holds `A1/ast::Rule`, but `register_ids` reads only
/// `ID`-headed tables, so its row would drop out of uniqueness, well-formedness,
/// and coverage while the untouched site table kept `validate_populated`
/// non-empty. The case therefore covers a hole a section-level emptiness check
/// cannot reach, and reports the header that opens it.
#[test]
fn rejects_a_register_table_whose_header_is_not_id() -> Result<()> {
    let inventory = inventory_with_register_item_and_unchanged("None yet.\n").replace(
        "| ID | Item | Visibility | Derives | Location |",
        "| Identifier | Item | Visibility | Derives | Location |",
    );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "should be `ID`-headed")?;
    Ok(())
}
