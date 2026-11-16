//! Harvest the trace matrix's task, register, and exclusion evidence.
//!
//! The trace matrix is the one section whose subsections carry different kinds
//! of claim: `Changes` and `Consumes` hold table rows, `Excluded` holds
//! reason-bearing bullets, and `Unchanged` holds identifier bullets. Each shape
//! is harvested on its own terms here, and each one refuses to read prose, so
//! the four harvesters share a single rule — a claim must be made by the form
//! the subsection defines — even though their return types differ.
//!
//! Within a task row the rule narrows again: a claim is made by a named column,
//! not by the row's text. Every reader here therefore selects a row through
//! `task_table_rows` and then reads the cell its question belongs to, so a
//! token parked in a neighbouring column cannot answer for it.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};

use super::markdown::{first_cell, harvest_body, is_table_separator, task_ids};
use super::obligations::{
    TRACE_TASK_HEADER, TraceRow, is_canonical_trace_header, parse_trace_row, row_obligations,
};
use super::register::is_well_formed_register_id;

/// Collect task identifiers from the first column of the trace-matrix tables.
///
/// Only the `Changes` and `Consumes` subsections are read. Scanning the whole
/// trace region would let a `Task` table nested under `Excluded`, or under a
/// deeper heading inside it, satisfy task coverage even though the exclusion
/// harvester had already refused that nested content. The register and
/// obligation readers restrict the same way, so one rule covers all three.
///
/// # Errors
///
/// Returns an error when a `Task`-headed table's header is not the canonical
/// one, for the reason `is_canonical_trace_header` gives.
pub(super) fn trace_table_tasks(trace: &str) -> Result<BTreeSet<String>> {
    Ok(trace_rows(trace)?
        .into_iter()
        .flat_map(|row| task_ids(first_cell(row).unwrap_or_default()))
        .collect())
}

/// Iterate the `Changes` and `Consumes` harvest bodies of the trace matrix.
fn trace_subsections(trace: &str) -> impl Iterator<Item = &str> + '_ {
    ["Changes", "Consumes"]
        .into_iter()
        .filter_map(move |heading| harvest_body(trace, heading))
}

/// Collect the task rows of every `Task`-headed table in the trace matrix.
///
/// Every reader that asks a question of a task row goes through this, so they
/// cannot disagree about which rows are evidence — nor about whether the header
/// they all read by position is the one they assume.
///
/// # Errors
///
/// Returns an error when a `Task`-headed table's header is not the canonical
/// `| Task | Register items | Findings | Obligations |`. The row parser assigns
/// columns by position, so a header naming the same cells in another order
/// would hand the `Findings` cell to `Register items` and let coverage read a
/// column the row never wrote. Refusing the header is what turns that misread
/// into a failure instead of a silent answer to the wrong question.
fn trace_rows(trace: &str) -> Result<Vec<&str>> {
    let mut rows = Vec::new();
    for subsection in trace_subsections(trace) {
        rows.extend(task_table_rows(subsection)?);
    }
    Ok(rows)
}

/// Collect the rows of one trace subsection's `Task`-headed tables.
///
/// Task coverage and reference coverage share this selection so the two cannot
/// disagree about which rows are evidence. A row counts only when it belongs to
/// a `Task`-headed table *and* its first cell names a roadmap task: without the
/// first condition an isolated pipe-delimited line under `### Changes` becomes a
/// reference source, and with it a row naming no task would still be read as
/// one. Both shapes let a row that traces nothing satisfy register coverage.
///
/// # Errors
///
/// Returns an error when a line that opens a `Task`-headed table is not the
/// canonical header, because every later row of that table is read against it.
fn task_table_rows(subsection: &str) -> Result<Vec<&str>> {
    let mut in_task_table = false;
    let mut rows = Vec::new();
    for line in subsection.lines() {
        let Some(cell) = first_cell(line) else {
            in_task_table = false;
            continue;
        };
        if cell == TRACE_TASK_HEADER {
            ensure!(
                is_canonical_trace_header(line),
                concat!(
                    "trace table header should read `| Task | Register items | Findings | ",
                    "Obligations |`, the order the reader assigns columns by, but found: {line}"
                )
            );
            in_task_table = true;
        } else if is_task_row(in_task_table, cell) {
            rows.push(line);
        }
    }
    Ok(rows)
}

/// Report whether a table row names at least one roadmap task.
fn is_task_row(in_task_table: bool, cell: &str) -> bool {
    in_task_table && !is_table_separator(cell) && !task_ids(cell).is_empty()
}

/// Map each traced task to the obligation identifiers its row names.
///
/// Selection is `task_table_rows`, the same one `trace_table_tasks` and
/// `trace_column` use, so a row cannot be a task for one check and a non-row for
/// another. A task appearing in both `Changes` and `Consumes` keeps the union:
/// the two tables describe one task's change and consumption duties, and
/// dropping either would report a bound obligation as missing.
///
/// # Errors
///
/// Returns an error when any selected row is not the width its header declares,
/// because a malformed row cannot be read as evidence for the binding.
pub(super) fn trace_task_obligations(trace: &str) -> Result<BTreeMap<String, BTreeSet<String>>> {
    let mut by_task = BTreeMap::<String, BTreeSet<String>>::new();
    for row in trace_rows(trace)? {
        let obligations = row_obligations(row)?;
        for task in task_ids(first_cell(row).unwrap_or_default()) {
            by_task
                .entry(task)
                .or_default()
                .extend(obligations.iter().cloned());
        }
    }
    Ok(by_task)
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

/// Collect the `Register items` cells of the trace matrix's task rows.
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
///
/// One column is read, not the whole row. A row carries four kinds of claim
/// side by side, and a token in a neighbour's cell is not a claim about this
/// one: `| 26.1.2 | -- | A1/ast::Rule | CO-1 |` puts the item in `Findings`
/// and says nothing about what the task changes, however plainly it names it.
/// Reading the row as text would let any column answer for any other, which is
/// the fail-open this rule exists to refuse — and it is the shape a row that
/// traces nothing takes when someone pastes an identifier one cell too far
/// right.
///
/// # Errors
///
/// Returns an error when a selected row is not the width its header declares,
/// because a shifted row cannot be read as evidence for any column.
pub(super) fn trace_register_references(trace: &str) -> Result<String> {
    trace_column(trace, |row| &row.register_items)
}

/// Collect the `Obligations` cells of the trace matrix's task rows.
///
/// Selection follows `trace_register_references` exactly — the same
/// subsections, the same `Task`-headed tables, the same width check — and
/// differs only in the cell, so obligation coverage and register coverage
/// cannot disagree about which rows are evidence.
///
/// # Errors
///
/// Returns an error when a selected row is not the width its header declares.
pub(super) fn trace_obligation_references(trace: &str) -> Result<String> {
    trace_column(trace, |row| &row.obligations)
}

/// Collect one cell from every trace-matrix task row.
///
/// # Errors
///
/// Returns an error when a selected row is not the width its header declares.
fn trace_column(trace: &str, select: impl Fn(&TraceRow) -> &str) -> Result<String> {
    let mut cells = String::new();
    for row in trace_rows(trace)? {
        cells.push_str(select(&parse_trace_row(row)?));
        cells.push('\n');
    }
    Ok(cells)
}

/// Collect register identifiers from the trace matrix's `Unchanged` bullets.
///
/// A register item no task changes is listed as `- A1/ast::Rule: reason`, so
/// only an identifier that opens a bullet counts. A sentence that merely names
/// an identifier would let prose satisfy coverage, the same fail-open that
/// `trace_column` closes for the introduction and the exclusions. Only exact
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
