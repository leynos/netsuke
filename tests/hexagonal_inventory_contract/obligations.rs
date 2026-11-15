//! Bind each obligation's declared tasks to trace rows that name it.
//!
//! `validate_obligations` answers one question: does this obligation appear
//! *somewhere* in the trace matrix? That leaves the inverse unasked, and it is
//! the one a reader follows. An obligation says which tasks must preserve it in
//! its own `Tasks:` clause; a reader opening that task's row then sees the
//! obligations column and trusts it to be the same set. When the two disagree,
//! the row silently drops a preservation duty the obligation assigns to it, and
//! no existing check notices because the obligation is still referenced by some
//! other row.
//!
//! The binding is deliberately one-directional. A row may name obligations its
//! own clause does not bind — `28.1.2` maps every exception, and a task that
//! changes a public item names `CO-13` without being on that obligation's task
//! list — and demanding equality would reject both. What must hold is that no
//! bound task omits its charge.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use super::markdown::{regex_values, task_ids};
use super::trace::trace_task_obligations;

/// The identifier form the compatibility obligations and trace matrix share.
pub(super) const OBLIGATION_PATTERN: &str = r"\bCO-\d+\b";

/// Require every task an obligation binds to name it in its trace row.
///
/// # Errors
///
/// Returns an error when an obligation has no `Tasks:` clause, names no task in
/// that clause, or binds a task whose trace row omits the obligation or does
/// not exist.
///
/// The `Tasks:` clause is required rather than optional. An obligation that
/// omits it would bind nothing, and the loop over its tasks would then visit
/// nothing and pass — the same vacuous pass this contract refuses elsewhere.
pub(super) fn validate_obligation_tasks(obligations: &str, trace: &str) -> Result<()> {
    let rows = trace_task_obligations(trace);
    for entry in obligation_entries(obligations) {
        let (identifier, tasks) = declared_tasks(&entry)?;
        for task in tasks {
            let named = rows
                .get(&task)
                .is_some_and(|named| named.contains(&identifier));
            ensure!(
                named,
                concat!(
                    "trace row for task {} should name obligation {}, ",
                    "which that obligation's `Tasks:` clause binds"
                ),
                task,
                identifier
            );
        }
    }
    Ok(())
}

/// Split the obligations section into one entry per obligation bullet.
///
/// A wrapped bullet continues on indented lines, and any other line ends the
/// entry. Ending on an unindented line matters: without it the prose that
/// follows the list would be appended to the final obligation and could donate
/// task identifiers that the obligation never bound.
fn obligation_entries(obligations: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut current = None::<String>;
    for line in obligations.lines() {
        if is_obligation_start(line) {
            finish_entry(&mut current, &mut entries);
            current = Some(line.trim_start().to_owned());
        } else if is_continuation(line) {
            if let Some(entry) = current.as_mut() {
                entry.push('\n');
                entry.push_str(line.trim());
            }
        } else {
            finish_entry(&mut current, &mut entries);
        }
    }
    finish_entry(&mut current, &mut entries);
    entries
}

/// Report whether a line opens a compatibility-obligation bullet.
fn is_obligation_start(line: &str) -> bool {
    line.trim_start()
        .strip_prefix("- ")
        .is_some_and(|bullet| bullet.starts_with("CO-"))
}

/// Report whether a line continues the bullet above it.
fn is_continuation(line: &str) -> bool {
    line.starts_with(' ') || line.starts_with('\t')
}

/// Move a completed obligation entry into the result list.
fn finish_entry(current: &mut Option<String>, entries: &mut Vec<String>) {
    if let Some(entry) = current.take() {
        entries.push(entry);
    }
}

/// Read one obligation's identifier and the tasks its `Tasks:` clause binds.
///
/// # Errors
///
/// Returns an error when the entry names no obligation identifier, has no
/// `Tasks:` clause, or names no task after it.
fn declared_tasks(entry: &str) -> Result<(String, BTreeSet<String>)> {
    let identifier = regex_values(OBLIGATION_PATTERN, entry)
        .into_iter()
        .next()
        .with_context(|| format!("obligation entry should name its identifier: {entry}"))?;
    let clause = entry.split_once("Tasks:").with_context(|| {
        format!("obligation {identifier} should declare the tasks it binds in a `Tasks:` clause")
    })?;
    let tasks = task_ids(clause.1);
    ensure!(
        !tasks.is_empty(),
        "obligation {identifier} should name at least one task after `Tasks:`"
    );
    Ok((identifier, tasks))
}

/// Read the obligation identifiers a single trace row names.
///
/// Only the row's final cell counts. The register and finding cells name
/// identifiers of their own, and reading them here would let one obligation
/// identifier sitting in the wrong column satisfy a binding it never names.
pub(super) fn row_obligations(row: &str) -> BTreeSet<String> {
    row.trim()
        .strip_suffix('|')
        .and_then(|cells| cells.rsplit_once('|'))
        .map_or_else(BTreeSet::new, |(_, cell)| {
            regex_values(OBLIGATION_PATTERN, cell)
        })
}
