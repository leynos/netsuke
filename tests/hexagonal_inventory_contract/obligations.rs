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
use std::sync::LazyLock;

use anyhow::{Context, Result, bail, ensure};
use regex::Regex;

use super::markdown::{bullet_entries, regex_values, task_ids};
use super::trace::trace_task_obligations;

/// The identifier form the compatibility obligations and trace matrix share.
pub(super) const OBLIGATION_PATTERN: &str = r"\bCO-\d+\b";

/// The compiled form of `OBLIGATION_PATTERN`.
///
/// `is_obligation_start` asks this question of every line in the section, so a
/// fresh compile per line would repeat the same work for the same answer.
static OBLIGATION_IDENTIFIER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(OBLIGATION_PATTERN).expect("obligation identifier pattern should compile")
});

/// The label form that may open a compatibility-obligation bullet.
///
/// Anchored, and applied to the text after `- `, so it matches the bullet's
/// label and nothing else. `regex_values` is deliberately not used: it scans
/// the whole entry and cannot tell a label from a cross-reference.
const OBLIGATION_LABEL: &str = r"^(?P<identifier>CO-\d+)\s*:";

/// The compiled form of `OBLIGATION_LABEL`.
static OBLIGATION_LABEL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(OBLIGATION_LABEL).expect("obligation label pattern should compile")
});

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
    let captured = OBLIGATION_LABEL_PATTERN
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
/// The collector is `markdown::bullet_entries`, shared with the landed-work
/// rule so the two cannot drift on where an entry ends. Only the predicate
/// differs, and it differs for the reason `is_obligation_start` gives.
fn obligation_entries(obligations: &str) -> Vec<String> {
    bullet_entries(obligations, is_obligation_start)
}

/// Report whether a line opens a compatibility-obligation bullet.
///
/// A bullet whose label this parser cannot read still opens an entry, so the
/// label check in `declared_tasks` reports it rather than letting it fall
/// through as prose and bind nothing. That is why the test is "carries a
/// `CO-` identifier anywhere" rather than "opens with a label this parser
/// accepts": enumerating the readable prefixes would reopen the fail-open for
/// every decoration missing from the list. A bold label, `- **CO-1**: …`,
/// renders as an obligation in Markdown and is reported here; matching only
/// bare and backticked prefixes let it be read as prose, bind nothing, and
/// still satisfy the global text check.
fn is_obligation_start(line: &str) -> bool {
    line.trim_start()
        .strip_prefix("- ")
        .is_some_and(|bullet| OBLIGATION_IDENTIFIER.is_match(bullet))
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

/// The two claim cells of a trace row that a coverage rule reads.
///
/// The row's header is `| Task | Register items | Findings | Obligations |`.
/// `Task` is read from the first cell by the selectors that decide whether a
/// line is a row at all, and `Findings` is read by no rule here, so only the
/// two columns a rule asks a question of are carried. Parsing names them, so a
/// caller asks for the claim it wants rather than for a column index and no
/// reader can take the wrong column by miscounting pipes.
pub(super) struct TraceRow {
    /// The `Register items` cell: what the task changes or consumes.
    pub(super) register_items: String,
    /// The `Obligations` cell: what the task must preserve.
    pub(super) obligations: String,
}

/// Split one trace row into the claim cells the coverage rules read.
///
/// The width check lives here, so every reader refuses a row it cannot read
/// rather than reading a neighbour's column. That refusal is the point: after an
/// omitted cell every later cell shifts left, so `| 26.1.2 | CO-1 |` would hand
/// `Register items` the value `CO-1` and leave `Obligations` absent.
///
/// # Errors
///
/// Returns an error when `row` is not a pipe-delimited row of `TRACE_COLUMNS`
/// cells, naming the row and the width it carries.
pub(super) fn parse_trace_row(row: &str) -> Result<TraceRow> {
    let cells = table_cells(row);
    let width = cells.len();
    let Ok(cells) = <[String; TRACE_COLUMNS]>::try_from(cells) else {
        bail!(
            concat!(
                "trace row should carry {} cells, the width its `Task`-headed header ",
                "declares, but carries {}: {}"
            ),
            TRACE_COLUMNS,
            width,
            row
        );
    };
    let [_task, register_items, _findings, obligations] = cells;
    Ok(TraceRow {
        register_items,
        obligations,
    })
}

/// Read the obligation identifiers a single trace row names.
///
/// Only the `Obligations` cell counts, and only when the row is the width its
/// header declares. Restricting the cell refuses the way an obligation is most
/// easily named in the wrong place: the register and findings cells carry
/// identifiers of their own, so reading the whole row would let one parked
/// there satisfy a binding it never names.
///
/// # Errors
///
/// Returns an error when `row` is not a pipe-delimited row of `TRACE_COLUMNS`
/// cells. Reporting the malformed row is the point: a binding check that
/// returned an empty set would pass a bound task whose row names nothing.
pub(super) fn row_obligations(row: &str) -> Result<BTreeSet<String>> {
    let parsed = parse_trace_row(row)?;
    Ok(regex_values(OBLIGATION_PATTERN, &parsed.obligations))
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
