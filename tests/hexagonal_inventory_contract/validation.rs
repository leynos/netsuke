//! Check inventory claims against the pinned roadmap and related sections.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::markdown::{
    declared_pin, excluded_tasks, finding_entries, is_well_formed_register_id, pins,
    reference_covers, regex_values, register_ids, section, task_ids, trace_table_tasks,
};
use super::roadmap::{dependents_of, roadmap_tasks};

/// Validate the inventory's pin, register, trace matrix, and finding owners.
///
/// # Errors
///
/// Returns an error when the inventory disagrees with its roadmap or omits a
/// required structural relationship.
pub(super) fn validate_inventory(inventory: &str, roadmap: &str) -> Result<()> {
    validate_pin(inventory)?;
    validate_required_sections(inventory)?;
    validate_trace_coverage(inventory, roadmap)?;
    validate_register(inventory)?;
    validate_obligations(inventory)?;
    validate_findings(inventory, roadmap)
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
            section(inventory, heading).is_some(),
            "inventory should contain the `## {heading}` section"
        );
    }
    Ok(())
}

/// Ensure the trace covers every roadmap task that depends on the inventory task.
///
/// # Errors
///
/// Returns an error for unknown roadmap tasks, reasonless exclusions, or a
/// missing transitive dependent.
fn validate_trace_coverage(inventory: &str, roadmap: &str) -> Result<()> {
    let tasks = roadmap_tasks(roadmap);
    ensure!(
        tasks.contains_key("26.1.1"),
        "roadmap should contain task 26.1.1"
    );
    let trace = section(inventory, "Trace matrix").unwrap_or_default();
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
    let register = section(inventory, "Register").unwrap_or_default();
    let register_document = format!("## Register\n{register}");
    let register_identifiers = register_ids(&register_document);
    let unique: BTreeSet<&str> = register_identifiers.iter().map(String::as_str).collect();
    ensure!(
        unique.len() == register_identifiers.len(),
        "register identifiers should be unique"
    );
    let trace = section(inventory, "Trace matrix").unwrap_or_default();
    for identifier in &register_identifiers {
        ensure!(
            is_well_formed_register_id(identifier),
            "register identifier {identifier:?} is malformed"
        );
        ensure!(
            reference_covers(identifier, trace),
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
    let obligations = section(inventory, "Compatibility obligations").unwrap_or_default();
    let trace = section(inventory, "Trace matrix").unwrap_or_default();
    for obligation in regex_values(r"\bCO-\d+\b", obligations) {
        ensure!(
            trace.contains(&obligation),
            "compatibility obligation {obligation} has no trace-matrix reference"
        );
    }
    Ok(())
}

/// Check that every finding names an existing owning roadmap task.
///
/// # Errors
///
/// Returns an error when a finding has no owner or names an unknown task.
fn validate_findings(inventory: &str, roadmap: &str) -> Result<()> {
    let tasks = roadmap_tasks(roadmap);
    let findings = section(inventory, "Findings").unwrap_or_default();
    for entry in finding_entries(findings) {
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
