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
//!
//! Every reader here refuses a shape it cannot read rather than accepting the
//! nearest interpretation. An entry whose label is not a bare identifier, and a
//! row that is not the width its header declares, are both errors: a binding
//! check that silently skips an entry, or reads a neighbouring column, reports
//! success for a row that names nothing.

use std::collections::BTreeSet;

use anyhow::{Context, Result, bail, ensure};

use super::markdown::{regex_values, task_ids};
use super::trace::trace_task_obligations;

/// The identifier form the compatibility obligations and trace matrix share.
pub(super) const OBLIGATION_PATTERN: &str = r"\bCO-\d+\b";

/// The label form that may open a compatibility-obligation bullet.
///
/// Anchored, and applied to the text after `- `, so it matches the bullet's
/// label and nothing else. `regex_values` is deliberately not used: it scans
/// the whole entry and cannot tell a label from a cross-reference.
const OBLIGATION_LABEL: &str = r"^(?P<identifier>CO-\d+)\s*:";

/// Read the identifier a compatibility-obligation bullet opens with.
///
/// The label is the obligation's name; a reference later in the text is not.
/// `- CO-2: Preserve CO-1 too.` declares `CO-2`, and reading it as `CO-1` — the
/// lowest identifier anywhere in the entry — would check the wrong obligation,
/// leaving `CO-2`'s own binding unverified.
///
/// # Errors
///
/// Returns an error when the bullet does not open with a bare `CO-<number>:`
/// label. A backticked or otherwise decorated label is refused rather than
/// skipped, because a form this parser cannot read must not silently bind
/// nothing.
fn obligation_label(line: &str) -> Result<&str> {
    let trimmed = line.trim_start();
    let bullet = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    let pattern =
        regex::Regex::new(OBLIGATION_LABEL).expect("obligation label pattern should compile");
    let captured = pattern
        .captures(bullet)
        .and_then(|captures| captures.name("identifier"))
        .map(|matched| matched.as_str());
    captured.with_context(|| {
        format!("obligation bullet should open with a bare `CO-<number>:` label: {line}")
    })
}

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
    let rows = trace_task_obligations(trace)?;
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
///
/// Each bullet is pushed the moment it opens, and a continuation is appended to
/// the last entry only while a bullet is open. Keeping the open/closed fact in
/// `is_open` means the append never has to ask whether an entry exists.
fn obligation_entries(obligations: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut open = None::<String>;
    for line in obligations.lines() {
        if is_obligation_start(line) {
            closed_entry(open.take(), &mut entries);
            open = Some(line.trim_start().to_owned());
        } else if is_continuation(line) {
            extend_open(&mut open, line);
        } else {
            closed_entry(open.take(), &mut entries);
        }
    }
    closed_entry(open.take(), &mut entries);
    entries
}

/// Append an indented continuation line to the entry it belongs to.
///
/// A continuation with no open entry belongs to no obligation — it is the
/// pre-list prose, an indented block under some earlier heading — and must be
/// dropped rather than started as an entry, or it would be read as one.
fn extend_open(open: &mut Option<String>, line: &str) {
    if let Some(text) = open.as_mut() {
        text.push('\n');
        text.push_str(line.trim());
    }
}

/// Push a completed obligation entry onto the result list.
fn closed_entry(entry: Option<String>, entries: &mut Vec<String>) {
    entries.extend(entry);
}

/// Report whether a line opens a compatibility-obligation bullet.
///
/// A bullet whose label this parser cannot read still opens an entry, so the
/// label check in `declared_tasks` reports it rather than letting it fall
/// through as prose and bind nothing.
fn is_obligation_start(line: &str) -> bool {
    line.trim_start()
        .strip_prefix("- ")
        .is_some_and(|bullet| bullet.starts_with("CO-") || bullet.starts_with("`CO-"))
}

/// Report whether a line continues the bullet above it.
fn is_continuation(line: &str) -> bool {
    line.starts_with(' ') || line.starts_with('\t')
}

/// Read one obligation's identifier and the tasks its `Tasks:` clause binds.
///
/// # Errors
///
/// Returns an error when the entry does not open with a bare obligation label,
/// has no `Tasks:` clause, or names no task after it.
fn declared_tasks(entry: &str) -> Result<(String, BTreeSet<String>)> {
    let label = obligation_label(entry)?;
    let identifier = label.to_owned();
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

/// The column count every trace task row must carry.
///
/// `| Task | Register items | Findings | Obligations |` is the one header both
/// trace tables use, so a row of any other width is malformed.
pub(super) const TRACE_COLUMNS: usize = 4;

/// Read the obligation identifiers a single trace row names.
///
/// Only the `Obligations` column counts, and only when the row is the width its
/// header declares. Both restrictions refuse a different way of naming an
/// obligation in the wrong place:
///
/// - The register and findings cells carry identifiers of their own, so reading
///   the whole row would let one parked there satisfy a binding it never names.
/// - Taking the last supplied cell as `Obligations` would do the same for a
///   short row: `| 26.1.2 | CO-1 |` puts `CO-1` in `Register items` and leaves
///   `Obligations` absent, yet its last cell is `CO-1`.
///
/// # Errors
///
/// Returns an error when `row` is not a pipe-delimited row of `TRACE_COLUMNS`
/// cells. Reporting the malformed row is the point: a binding check that
/// returned an empty set would pass a bound task whose row names nothing.
pub(super) fn row_obligations(row: &str) -> Result<BTreeSet<String>> {
    let cells = table_cells(row);
    ensure!(
        cells.len() == TRACE_COLUMNS,
        concat!(
            "trace row should carry {} cells, the width its `Task`-headed header ",
            "declares, but carries {}: {}"
        ),
        TRACE_COLUMNS,
        cells.len(),
        row
    );
    let Some(obligations) = cells.last() else {
        bail!("trace row should carry an `Obligations` cell: {row}");
    };
    Ok(regex_values(OBLIGATION_PATTERN, obligations))
}

/// Split a pipe-delimited Markdown table row into its trimmed cells.
///
/// A trailing pipe closes the row rather than opening an empty final cell, so
/// `| a | b |` yields two cells. An escaped `\|` is a literal pipe inside a
/// cell, which Markdown renders as text and this split must not treat as a
/// boundary; the inventory's trace tables contain none, but a cell that gained
/// one must not shift every column after it.
fn table_cells(row: &str) -> Vec<String> {
    let trimmed = row.trim();
    let body = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let inner = body.strip_suffix('|').unwrap_or(body);
    // A `|` inside a cell is escaped as `\|`, so a boundary is a pipe whose
    // preceding character is not a backslash. Tracking that character rather
    // than its byte index keeps every slice on a char boundary.
    let mut cells = Vec::new();
    let mut pending = String::new();
    let mut previous = None;
    for character in inner.chars() {
        if character == '|' && previous != Some('\\') {
            cells.push(pending.trim().to_owned());
            pending.clear();
        } else {
            pending.push(character);
        }
        previous = Some(character);
    }
    cells.push(pending.trim().to_owned());
    cells
}
