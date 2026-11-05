//! Check inventory claims against the pinned roadmap and related sections.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::markdown::{
    HeadingLevel, declared_pin, excluded_tasks, finding_entries, heading_body,
    is_well_formed_register_id, pins, reference_covers, regex_values, register_ids, task_ids,
    trace_rows, trace_table_tasks,
};
use super::roadmap::{dependents_of, roadmap_tasks};

/// The identifier form the compatibility obligations and trace matrix share.
const OBLIGATION_PATTERN: &str = r"\bCO-\d+\b";

/// Require the real inventory to populate the sections whose checks iterate.
///
/// # Errors
///
/// Returns an error when the register, obligations, or findings section parses
/// to no entries.
///
/// `validate_inventory` must accept the skeleton fixture's empty sections, so a
/// renamed table header or a changed identifier format would leave its loops
/// with nothing to visit and turn every check into a silent pass. Holding the
/// real document to non-emptiness is what keeps those loops live.
pub(super) fn validate_populated(inventory: &str) -> Result<()> {
    ensure!(
        !register_ids(inventory).is_empty(),
        "the real inventory should register at least one item"
    );
    ensure!(
        !regex_values(
            OBLIGATION_PATTERN,
            section(inventory, "Compatibility obligations")
        )
        .is_empty(),
        "the real inventory should record at least one compatibility obligation"
    );
    ensure!(
        !finding_entries(section(inventory, "Findings")).is_empty(),
        "the real inventory should record at least one finding"
    );
    Ok(())
}

/// Return a required second-level section body, or an empty string.
fn section<'text>(inventory: &'text str, heading: &str) -> &'text str {
    heading_body(inventory, heading, HeadingLevel::Section).unwrap_or_default()
}

/// Validate the inventory's pin, register, trace matrix, and finding owners.
///
/// # Errors
///
/// Returns an error when the inventory disagrees with its roadmap or omits a
/// required structural relationship.
pub(super) fn validate_inventory(inventory: &str, roadmap: &str) -> Result<()> {
    let validation = InventoryValidation { inventory, roadmap };
    validate_pin(inventory)?;
    validate_required_sections(inventory)?;
    validation.validate_trace_coverage()?;
    validate_register(inventory)?;
    validate_obligations(inventory)?;
    validation.validate_findings()
}

/// Carry the two documents whose claims must be checked together.
struct InventoryValidation<'text> {
    inventory: &'text str,
    roadmap: &'text str,
}

impl InventoryValidation<'_> {
    /// Check that the trace covers every roadmap task that depends on the inventory.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown roadmap tasks, reasonless exclusions, or a
    /// missing transitive dependent.
    fn validate_trace_coverage(&self) -> Result<()> {
        let tasks = roadmap_tasks(self.roadmap);
        ensure!(
            tasks.contains_key("26.1.1"),
            "roadmap should contain task 26.1.1"
        );
        let trace = section(self.inventory, "Trace matrix");
        validate_known_trace_tasks(trace, &tasks)?;
        validate_exclusions(trace, &tasks)?;

        let mut covered = trace_table_tasks(trace);
        covered.extend(excluded_tasks(trace).keys().cloned());
        let missing: BTreeSet<String> = dependents_of(&tasks, "26.1.1")
            .difference(&covered)
            .cloned()
            .collect();
        ensure!(
            missing.is_empty(),
            "trace matrix should cover every dependent task; missing: {missing:?}"
        );
        Ok(())
    }

    /// Check that every finding names an existing owning roadmap task.
    ///
    /// # Errors
    ///
    /// Returns an error when a finding has no owner or names an unknown task.
    fn validate_findings(&self) -> Result<()> {
        let tasks = roadmap_tasks(self.roadmap);
        for entry in finding_entries(section(self.inventory, "Findings")) {
            let owners = task_ids(&entry);
            ensure!(
                !owners.is_empty(),
                "finding should name an owning roadmap task: {entry}"
            );
            for owner in owners {
                ensure!(
                    tasks.contains_key(&owner),
                    "finding owner {owner} is absent from the roadmap"
                );
            }
        }
        Ok(())
    }
}

/// Confirm the inventory declares exactly one valid revision pin.
///
/// # Errors
///
/// Returns an error when the pin is absent, duplicated, malformed, or mixed
/// with another hexadecimal revision.
fn validate_pin(inventory: &str) -> Result<()> {
    let pin_lines = inventory
        .lines()
        .filter(|line| line.starts_with("Pin:"))
        .count();
    ensure!(
        pin_lines == 1,
        "inventory should declare exactly one `Pin:` line"
    );
    let pin = declared_pin(inventory).context("inventory should declare a non-empty Pin:")?;
    ensure!(
        pins(inventory) == BTreeSet::from([pin]),
        "inventory should contain only its declared lowercase 40-character pin"
    );
    Ok(())
}

/// Require the structural sections used by later inventory consumers.
///
/// # Errors
///
/// Returns an error when a required second-level heading is absent.
fn validate_required_sections(inventory: &str) -> Result<()> {
    for heading in [
        "Scope and definitions",
        "Method",
        "Register",
        "Landed work",
        "Findings",
        "Compatibility obligations",
        "Trace matrix",
        "Consuming this inventory",
        "Refreshing this inventory",
    ] {
        ensure!(
            heading_body(inventory, heading, HeadingLevel::Section).is_some(),
            "inventory should contain the `## {heading}` section"
        );
    }
    Ok(())
}

/// Reject trace references that do not identify tasks in the pinned roadmap.
///
/// # Errors
///
/// Returns an error when a trace reference does not identify a roadmap task.
fn validate_known_trace_tasks(
    trace: &str,
    tasks: &BTreeMap<String, BTreeSet<String>>,
) -> Result<()> {
    for identifier in task_ids(trace) {
        ensure!(
            tasks.contains_key(&identifier),
            "trace matrix refers to task {identifier}, which is absent from the roadmap"
        );
    }
    Ok(())
}

/// Require every excluded roadmap task to have a non-empty explanation.
///
/// # Errors
///
/// Returns an error when an exclusion is unknown or has no explanation.
fn validate_exclusions(trace: &str, tasks: &BTreeMap<String, BTreeSet<String>>) -> Result<()> {
    for (identifier, reason) in excluded_tasks(trace) {
        ensure!(
            tasks.contains_key(&identifier),
            "excluded task {identifier} is absent from the roadmap"
        );
        ensure!(
            !reason.is_empty(),
            "excluded task {identifier} should give a reason"
        );
    }
    Ok(())
}

/// Check register identifiers and their trace references.
///
/// # Errors
///
/// Returns an error when identifiers are duplicated, malformed, or untraced.
fn validate_register(inventory: &str) -> Result<()> {
    let register = heading_body(inventory, "Register", HeadingLevel::Section).unwrap_or_default();
    let register_document = format!("## Register\n{register}");
    let register_identifiers = register_ids(&register_document);
    let unique: BTreeSet<&str> = register_identifiers.iter().map(String::as_str).collect();
    ensure!(
        unique.len() == register_identifiers.len(),
        "register identifiers should be unique"
    );
    let trace = heading_body(inventory, "Trace matrix", HeadingLevel::Section).unwrap_or_default();
    let references = trace_rows(trace);
    for identifier in &register_identifiers {
        ensure!(
            is_well_formed_register_id(identifier),
            "register identifier {identifier:?} is malformed"
        );
        ensure!(
            reference_covers(identifier, &references),
            "register identifier {identifier} is neither traced nor listed unchanged"
        );
    }
    Ok(())
}

/// Check that every compatibility obligation appears in the trace matrix.
///
/// # Errors
///
/// Returns an error when an obligation identifier is missing from the trace.
fn validate_obligations(inventory: &str) -> Result<()> {
    let obligations = section(inventory, "Compatibility obligations");
    let trace = section(inventory, "Trace matrix");
    let traced = regex_values(OBLIGATION_PATTERN, trace);
    for obligation in regex_values(OBLIGATION_PATTERN, obligations) {
        ensure!(
            traced.contains(&obligation),
            "compatibility obligation {obligation} has no trace-matrix reference"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Pin the vacuous-pass guard against inventories that omit each section.

    use anyhow::{Result, bail, ensure};

    use super::validate_populated;

    const POPULATED: &str = concat!(
        "# Inventory\n\n",
        "## Register\n\n",
        "### Type rows\n\n",
        "| ID | Item |\n",
        "| -- | -- |\n",
        "| A1/ast::Rule | Rule |\n\n",
        "## Findings\n\n",
        "- H1: An observed finding owned by 26.1.1.\n\n",
        "## Compatibility obligations\n\n",
        "- CO-1: Preserve the public surface.\n",
    );

    /// Return a fixture error when the guard accepts the inventory.
    fn expect_rejection(inventory: &str, expected: &str) -> Result<()> {
        match validate_populated(inventory) {
            Ok(()) => bail!("fixture should have been rejected for {expected:?}"),
            Err(error) => ensure!(
                format!("{error:#}").contains(expected),
                "expected error containing {expected:?}, found {error:#}"
            ),
        }
        Ok(())
    }

    #[test]
    fn accepts_an_inventory_populating_every_iterated_section() -> Result<()> {
        validate_populated(POPULATED)
    }

    #[test]
    fn rejects_an_inventory_without_register_rows() -> Result<()> {
        let inventory = POPULATED.replace("| A1/ast::Rule | Rule |\n", "");
        expect_rejection(&inventory, "should register at least one item")
    }

    #[test]
    fn rejects_an_inventory_without_compatibility_obligations() -> Result<()> {
        let inventory = POPULATED.replace("- CO-1: Preserve the public surface.\n", "Pending.\n");
        expect_rejection(&inventory, "at least one compatibility obligation")
    }

    #[test]
    fn rejects_an_inventory_without_findings() -> Result<()> {
        let inventory =
            POPULATED.replace("- H1: An observed finding owned by 26.1.1.\n", "Pending.\n");
        expect_rejection(&inventory, "at least one finding")
    }
}
