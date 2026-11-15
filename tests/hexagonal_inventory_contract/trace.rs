//! Harvest the trace matrix's task, register, and exclusion evidence.
//!
//! The trace matrix is the one section whose subsections carry different kinds
//! of claim: `Changes` and `Consumes` hold table rows, `Excluded` holds
//! reason-bearing bullets, and `Unchanged` holds identifier bullets. Each shape
//! is harvested on its own terms here, and each one refuses to read prose, so
//! the four harvesters share a single rule — a claim must be made by the form
//! the subsection defines — even though their return types differ.

use std::collections::{BTreeMap, BTreeSet};

use super::markdown::{first_cell, harvest_body, is_table_separator, task_ids};
use super::obligations::row_obligations;
use super::register::is_well_formed_register_id;

/// Collect task identifiers from the first column of the trace-matrix tables.
///
/// Only the `Changes` and `Consumes` subsections are read. Scanning the whole
/// trace region would let a `Task` table nested under `Excluded`, or under a
/// deeper heading inside it, satisfy task coverage even though the exclusion
/// harvester had already refused that nested content. `trace_rows` restricts
/// the obligation and register references the same way, so one rule covers all
/// three.
pub(super) fn trace_table_tasks(trace: &str) -> BTreeSet<String> {
    trace_subsections(trace)
        .flat_map(task_table_rows)
        .flat_map(|row| task_ids(first_cell(row).unwrap_or_default()))
        .collect()
}

/// Iterate the `Changes` and `Consumes` harvest bodies of the trace matrix.
fn trace_subsections(trace: &str) -> impl Iterator<Item = &str> + '_ {
    ["Changes", "Consumes"]
        .into_iter()
        .filter_map(move |heading| harvest_body(trace, heading))
}

/// Collect the rows of one trace subsection's `Task`-headed tables.
///
/// Task coverage and reference coverage share this selection so the two cannot
/// disagree about which rows are evidence. A row counts only when it belongs to
/// a `Task`-headed table *and* its first cell names a roadmap task: without the
/// first condition an isolated pipe-delimited line under `### Changes` becomes a
/// reference source, and with it a row naming no task would still be read as
/// one. Both shapes let a row that traces nothing satisfy register coverage.
fn task_table_rows(subsection: &str) -> Vec<&str> {
    let mut in_task_table = false;
    let mut rows = Vec::new();
    for line in subsection.lines() {
        let Some(cell) = first_cell(line) else {
            in_task_table = false;
            continue;
        };
        if cell == "Task" {
            in_task_table = true;
        } else if is_task_row(in_task_table, cell) {
            rows.push(line);
        }
    }
    rows
}

/// Report whether a table row names at least one roadmap task.
fn is_task_row(in_task_table: bool, cell: &str) -> bool {
    in_task_table && !is_table_separator(cell) && !task_ids(cell).is_empty()
}

/// Map each traced task to the obligation identifiers its row names.
///
/// Selection is `task_table_rows`, the same one `trace_table_tasks` and
/// `trace_rows` use, so a row cannot be a task for one check and a non-row for
/// another. A task appearing in both `Changes` and `Consumes` keeps the union:
/// the two tables describe one task's change and consumption duties, and
/// dropping either would report a bound obligation as missing.
pub(super) fn trace_task_obligations(trace: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut by_task = BTreeMap::<String, BTreeSet<String>>::new();
    let rows = trace_subsections(trace).flat_map(task_table_rows);
    for row in rows {
        let obligations = row_obligations(row);
        for task in task_ids(first_cell(row).unwrap_or_default()) {
            by_task
                .entry(task)
                .or_default()
                .extend(obligations.iter().cloned());
        }
    }
    by_task
}

/// Collect excluded tasks and the reason text following each identifier.
pub(super) fn excluded_tasks(trace: &str) -> BTreeMap<String, String> {
    let Some(excluded) = harvest_body(trace, "Excluded") else {
        return BTreeMap::new();
    };
    let mut tasks = BTreeMap::new();
    for line in excluded.lines() {
        let Some(identifier) = exclusion_identifier(line) else {
            continue;
        };
        tasks.insert(identifier.to_owned(), exclusion_reason(line, identifier));
    }
    tasks
}

/// Extract the single task identifier that opens an exclusion bullet.
///
/// An exclusion is a `- 26.1.2: reason` bullet. Scanning the whole line would
/// also register any task named in the reason, which can silently mark a
/// dependent that has no trace row as covered, so only the identifier before
/// the first colon counts.
fn exclusion_identifier(line: &str) -> Option<&str> {
    let bullet = line.trim_start().strip_prefix("- ")?;
    let (leading, _) = bullet.split_once(':')?;
    let candidate = leading.trim();
    let identifiers = task_ids(candidate);
    (identifiers.len() == 1 && identifiers.contains(candidate)).then_some(candidate)
}

/// Return the prose that follows an excluded task identifier.
fn exclusion_reason(line: &str, task: &str) -> String {
    line.split_once(task)
        .map_or_else(String::new, |(_, remainder)| {
            remainder
                .trim_start_matches(|character: char| {
                    matches!(character, ':' | '|' | '-' | '–' | '—' | ' ')
                })
                .trim()
                .to_owned()
        })
}

/// Return the trace matrix's table rows, dropping its prose.
///
/// The introduction explains wildcard semantics and names an area wildcard as
/// an example. Letting that prose satisfy coverage would mark an area as traced
/// even after every row naming it was deleted, so only the `Changes` and
/// `Consumes` subsections count. Their absence yields no references at all;
/// dropping the introduction by splitting at the first `### ` would hand the
/// `Excluded` and `Unchanged` prose the same power the introduction had.
/// `Unchanged` entries reach register coverage through `unchanged_ids`, which
/// accepts only the bullet form.
///
/// Within those subsections only the rows of a `Task`-headed table survive. A
/// sentence such as `No task currently covers A1/ast::Rule` sits directly under
/// `### Changes` and inside the harvest body, and counting it would let prose
/// make the claim that a table row has to make — the same fail-open this
/// function already closes for the introduction. An isolated pipe-delimited
/// line is prose in a table's clothing and is refused by the same rule: a row
/// that belongs to no `Task`-headed table traces nothing, however it is
/// punctuated.
pub(super) fn trace_rows(trace: &str) -> String {
    let mut rows = String::new();
    for row in trace_subsections(trace).flat_map(task_table_rows) {
        rows.push_str(row);
        rows.push('\n');
    }
    rows
}

/// Collect register identifiers from the trace matrix's `Unchanged` bullets.
///
/// A register item no task changes is listed as `- A1/ast::Rule: reason`, so
/// only an identifier that opens a bullet counts. A sentence that merely names
/// an identifier would let prose satisfy coverage, the same fail-open that
/// `trace_rows` closes for the introduction and the exclusions. Only exact
/// identifiers count; an unchanged entry names one item, so the wildcard forms
/// `reference_covers` accepts would be ambiguous here.
///
/// The identifier is the bullet's first word, not its text up to the first
/// colon: register identifiers themselves contain `::`, so splitting on a colon
/// would truncate `A1/ast::Rule` to `A1/ast` and reject every real entry.
pub(super) fn unchanged_ids(trace: &str) -> BTreeSet<String> {
    let Some(unchanged) = harvest_body(trace, "Unchanged") else {
        return BTreeSet::new();
    };
    unchanged
        .lines()
        .filter_map(unchanged_identifier)
        .map(str::to_owned)
        .collect()
}

/// Return the register identifier that opens an unchanged bullet.
fn unchanged_identifier(line: &str) -> Option<&str> {
    let bullet = line.trim_start().strip_prefix("- ")?;
    let candidate = bullet.split_whitespace().next()?.trim_end_matches(':');
    is_well_formed_register_id(candidate).then_some(candidate)
}

/// Check whether an exact register identifier or supported wildcard covers an ID.
pub(super) fn reference_covers(identifier: &str, references: &str) -> bool {
    references
        .split_whitespace()
        .map(|reference| reference.trim_matches(['|', ',', ';', '`', '(', ')']))
        .any(|reference| {
            reference == identifier
                || covers_area(reference, identifier)
                || covers_site(reference, identifier)
        })
}

/// Report whether an `<area>/*` token names a non-empty prefix of `identifier`.
///
/// The prefix must be non-empty: a bare `/*` strips to `""`, and every string
/// starts with `""`, so without this guard one malformed token would claim
/// coverage of every register identifier at once.
fn covers_area(reference: &str, identifier: &str) -> bool {
    reference
        .strip_suffix("/*")
        .is_some_and(|prefix| !prefix.is_empty() && identifier.starts_with(prefix))
}

/// Report whether an `<area>@*` token names a site on `identifier`.
fn covers_site(reference: &str, identifier: &str) -> bool {
    reference.strip_suffix("@*").is_some_and(|prefix| {
        identifier
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('@'))
    })
}
