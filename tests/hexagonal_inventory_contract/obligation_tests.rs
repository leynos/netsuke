//! Guard the task-to-obligation binding against every shape that waives it.
//!
//! `validate_obligation_tasks` exists because the older obligation check asked
//! only whether an obligation appears *somewhere*. Each rejection case here is
//! a way to satisfy that older question while breaking the binding, and each
//! traces the obligation from a row other than the one under test so the older
//! check cannot be the thing that fails. Reverting the rule makes every
//! rejection case pass, so each is a regression guard rather than an assertion
//! that happens to hold.

use anyhow::Result;
use rstest::rstest;

use super::fixtures::{
    SIMPLE_ROADMAP, VALID_SKELETON, expect_validation_error, inventory_with_one_register_item,
};
use super::validation::validate_inventory;

/// A row that names `CO-1` for a task no obligation binds.
///
/// Rejection cases add this alongside the row under test, so the obligation is
/// traced and `validate_obligations` passes; only the binding can then reject.
const ELSEWHERE: &str = "| 26.1.1 | -- | -- | CO-1 |";

/// One obligation binding 26.1.2 to `CO-1`.
const ONE_BINDING: &str = "## Compatibility obligations\n\n- CO-1: Preserve one. Tasks: 26.1.2.\n";

/// Insert `rows` into the skeleton's own `Task`-headed `Changes` table.
///
/// A row must belong to a `Task`-headed table to count as evidence, so the rows
/// go inside the existing one rather than beside it.
fn inventory_with_rows(obligations: &str, rows: &str) -> String {
    VALID_SKELETON
        .replace(
            "## Compatibility obligations\n\nPending EP-M3.",
            obligations,
        )
        .replace(
            "| Task | Register items | Findings | Obligations |\n| -- | -- | -- | -- |\n\n### Consumes",
            &format!(
                "| Task | Register items | Findings | Obligations |\n| -- | -- | -- | -- |\n{rows}\n\n### Consumes"
            ),
        )
}

/// Combine rows into one table body.
fn rows(rows: &[&str]) -> String {
    rows.join("\n")
}

/// Assert that an inventory carrying `obligations` and one row is accepted.
fn accepts(obligations: &str, row: &str) -> Result<()> {
    let inventory = inventory_with_rows(obligations, row);
    validate_inventory(&inventory, SIMPLE_ROADMAP)
}

#[test]
fn accepts_a_row_that_names_every_obligation_its_clause_binds() -> Result<()> {
    accepts(ONE_BINDING, "| 26.1.2 | -- | -- | CO-1 |")
}

/// A row may name obligations whose own clause does not bind that task.
///
/// The binding is one-directional on purpose: `28.1.2` maps every obligation,
/// and `27.1.1` names `CO-13` without appearing on `CO-13`'s task list.
/// Equality would reject both.
#[test]
fn accepts_a_row_naming_an_obligation_whose_clause_does_not_bind_it() -> Result<()> {
    let obligations = concat!(
        "## Compatibility obligations\n\n",
        "- CO-1: Preserve one. Tasks: 26.1.2.\n",
        "- CO-13: Promise no source compatibility. Tasks: 26.1.2.\n",
    );
    accepts(obligations, "| 26.1.2 | -- | -- | CO-1, CO-13 |")
}

/// The obligated task's row omits the obligation another row names.
///
/// This is the defect the older check could not see: `CO-1` is traced, so
/// `validate_obligations` is satisfied, yet 26.1.2 is bound by the clause and
/// its row does not name it.
#[test]
fn rejects_a_binding_satisfied_only_by_another_tasks_row() -> Result<()> {
    let inventory = inventory_with_rows(
        ONE_BINDING,
        &rows(&[ELSEWHERE, "| 26.1.2 | -- | -- | -- |"]),
    );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "should name obligation CO-1")?;
    Ok(())
}

/// A bound task with no row at all is the same omission as an empty cell.
#[test]
fn rejects_a_bound_task_that_has_no_trace_row() -> Result<()> {
    let inventory = inventory_with_rows(ONE_BINDING, ELSEWHERE);
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "should name obligation CO-1")?;
    Ok(())
}

/// An obligation with no `Tasks:` clause binds nothing and would pass vacuously.
#[test]
fn rejects_an_obligation_without_a_tasks_clause() -> Result<()> {
    let inventory = inventory_with_rows(
        "## Compatibility obligations\n\n- CO-1: Preserve one.\n",
        ELSEWHERE,
    );
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "`Tasks:` clause")?;
    Ok(())
}

#[rstest]
#[case::empty_clause("## Compatibility obligations\n\n- CO-1: Preserve one. Tasks: none yet.\n")]
#[case::clause_bound_to_no_roadmap_task(
    "## Compatibility obligations\n\n- CO-1: Preserve one. Tasks: the future work.\n"
)]
fn rejects_a_clause_that_binds_no_task(#[case] obligations: &str) -> Result<()> {
    let inventory = inventory_with_rows(obligations, ELSEWHERE);
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "at least one task")?;
    Ok(())
}

/// Prose after the obligation list must not donate tasks to the last bullet.
///
/// The paragraph names 26.1.3, which no clause binds, and which the roadmap
/// does not contain. If the entry absorbed it the rule would demand a row for
/// 26.1.3 and fail, so acceptance here proves the entry ends at the list.
#[test]
fn ignores_a_task_named_in_prose_after_the_obligation_list() -> Result<()> {
    let obligations = concat!(
        "## Compatibility obligations\n\n",
        "- CO-1: Preserve one. Tasks: 26.1.2.\n",
        "\n",
        "Further prose names 26.1.3, which no clause binds.\n",
    );
    accepts(obligations, "| 26.1.2 | -- | -- | CO-1 |")
}

/// An obligation identifier outside the obligations column is not evidence.
///
/// `row_obligations` reads only the final cell. Reading the whole row would let
/// a `CO-1` parked in the register column satisfy the binding, and here it does
/// satisfy `validate_obligations`, which reads the row as text.
#[test]
fn rejects_an_obligation_named_outside_the_obligations_column() -> Result<()> {
    let inventory = inventory_with_rows(ONE_BINDING, "| 26.1.2 | CO-1 | -- | -- |");
    expect_validation_error(&inventory, SIMPLE_ROADMAP, "should name obligation CO-1")?;
    Ok(())
}

/// Register coverage must still hold, so the case above fails for its own reason.
#[test]
fn register_coverage_is_unaffected_by_the_binding_rule() -> Result<()> {
    let inventory = inventory_with_one_register_item().replace(
        "## Compatibility obligations\n\nPending EP-M3.",
        ONE_BINDING,
    );
    expect_validation_error(
        &inventory,
        SIMPLE_ROADMAP,
        "neither traced nor listed unchanged",
    )?;
    Ok(())
}
