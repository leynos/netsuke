//! Parse the Markdown structures used by the inventory integrity contract.

use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;

use super::register::is_well_formed_register_id;

/// Select a heading depth supported by the inventory contract parser.
#[derive(Clone, Copy)]
pub(super) enum HeadingLevel {
    /// Match a second-level Markdown heading.
    Section,
    /// Match a third-level Markdown heading.
    Subsection,
}

/// Extract a heading's body up to the next heading at or above its level.
///
/// Deeper subheadings stay inside the body because they belong to the region,
/// not to the next one: `## Register` legitimately nests `#### A1 type rows`
/// inside each area's `###` heading, and slicing the region must not truncate
/// at the first of those.
///
/// Do not use this to harvest identifiers from a body. A deeper subheading's
/// *prose* is still prose, and treating it as part of the region would let it
/// supply a coverage claim that no table row or bullet makes. Use
/// `harvest_body` for that.
pub(super) fn heading_body<'text>(
    text: &'text str,
    heading: &str,
    level: HeadingLevel,
) -> Option<&'text str> {
    let marker = match level {
        HeadingLevel::Section => "## ",
        HeadingLevel::Subsection => "### ",
    };
    let mut offset = 0;
    let mut body_start = None;

    for line in text.split_inclusive('\n') {
        if body_start.is_some() && is_heading_at_or_above(line, level) {
            return text.get(body_start?..offset);
        }
        let title = line
            .trim_end_matches(['\r', '\n'])
            .strip_prefix(marker)
            .map(strip_heading_number);
        if title == Some(heading) {
            body_start = Some(offset + line.len());
        }
        offset += line.len();
    }

    body_start.and_then(|start| text.get(start..))
}

/// Extract a subsection's body for identifier harvesting.
///
/// The body is truncated at the first heading of any depth, so prose under a
/// deeper subheading can never contribute a task or register identifier. Each
/// harvester reads its body for claims that must be made by a table row or a
/// bullet; absorbing subheading prose would let an inventory satisfy coverage
/// with a sentence.
pub(super) fn harvest_body<'text>(text: &'text str, heading: &str) -> Option<&'text str> {
    let body = heading_body(text, heading, HeadingLevel::Subsection)?;
    body.get(..first_heading_offset(body))
}

/// Return the byte offset of the first Markdown heading in `body`.
fn first_heading_offset(body: &str) -> usize {
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        if is_heading(line) {
            return offset;
        }
        offset += line.len();
    }
    body.len()
}

/// Report whether a line is a Markdown ATX heading.
///
/// A `#` inside a fenced code block would have to open the line to be
/// mistaken for a heading, and the inventory's fences are indented, so a
/// line-prefix test is sufficient rather than a full Markdown parse.
fn is_heading(line: &str) -> bool {
    let depth = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    (1..=6).contains(&depth) && line.chars().nth(depth) == Some(' ')
}

/// Report whether a line is a Markdown heading at or above `level`.
fn is_heading_at_or_above(line: &str, level: HeadingLevel) -> bool {
    let depth = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    let required = match level {
        HeadingLevel::Section => 2,
        HeadingLevel::Subsection => 3,
    };
    (1..=required).contains(&depth) && line.chars().nth(depth) == Some(' ')
}

/// Remove a numeric list prefix from a Markdown heading.
fn strip_heading_number(title: &str) -> &str {
    match title.split_once(". ") {
        Some((number, rest)) if number.chars().all(|character| character.is_ascii_digit()) => rest,
        _ => title,
    }
}

/// Collect every lowercase, 40-character hexadecimal token in the text.
pub(super) fn pins(text: &str) -> BTreeSet<String> {
    regex_values(r"\b[a-f0-9]{40}\b", text)
}

/// Read the value from the inventory's `Pin:` line.
pub(super) fn declared_pin(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("Pin:").map(str::trim))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// Collect roadmap task identifiers, excluding versions and two-part labels.
pub(super) fn task_ids(text: &str) -> BTreeSet<String> {
    regex_values(r"\b2[6-9]\.\d+\.\d+\b", text)
}

/// Collect all matches from a constant regular-expression pattern.
///
/// # Panics
///
/// Panics when `pattern` does not compile. Every caller passes a compile-time
/// constant, so a failure is a programming error; returning an empty set would
/// turn a check such as obligation coverage into a silent, vacuous pass.
pub(super) fn regex_values(pattern: &str, text: &str) -> BTreeSet<String> {
    Regex::new(pattern)
        .expect("inventory contract regular expression should compile")
        .find_iter(text)
        .map(|matched| matched.as_str().to_owned())
        .collect()
}

/// Return first-column identifiers from the inventory's register tables.
pub(super) fn register_ids(text: &str) -> Vec<String> {
    let Some(register) = heading_body(text, "Register", HeadingLevel::Section) else {
        return Vec::new();
    };
    let mut reading_table = false;
    let mut identifiers = Vec::new();

    for line in register.lines() {
        let Some(cell) = first_cell(line) else {
            reading_table = false;
            continue;
        };
        if cell == "ID" {
            reading_table = true;
        } else if is_register_entry(reading_table, cell) {
            identifiers.push(cell.to_owned());
        }
    }
    identifiers
}

/// Identify a non-header, non-separator row in a register table.
fn is_register_entry(reading_table: bool, cell: &str) -> bool {
    reading_table && !is_table_separator(cell) && !cell.is_empty()
}

/// Return the first cell of a pipe-delimited Markdown table row.
fn first_cell(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix('|')?;
    let (cell, _) = rest.split_once('|')?;
    Some(cell.trim())
}

/// Identify a Markdown table's delimiter cell.
fn is_table_separator(cell: &str) -> bool {
    !cell.is_empty()
        && cell
            .chars()
            .all(|character| matches!(character, '-' | ':' | ' '))
}

/// Collect task identifiers from the first column of trace-matrix tables.
pub(super) fn trace_table_tasks(trace: &str) -> BTreeSet<String> {
    let mut in_task_table = false;
    let mut tasks = BTreeSet::new();
    for line in trace.lines() {
        let Some(cell) = first_cell(line) else {
            in_task_table = false;
            continue;
        };
        if cell == "Task" {
            in_task_table = true;
        } else if in_task_table && !is_table_separator(cell) {
            tasks.extend(task_ids(cell));
        }
    }
    tasks
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

/// Return the trace matrix's table rows, dropping its introductory prose.
///
/// The introduction explains wildcard semantics and names an area wildcard as
/// an example. Letting that prose satisfy coverage would mark an area as traced
/// even after every row naming it was deleted, so only the `Changes` and
/// `Consumes` subsections count. Their absence yields no references at all;
/// dropping the introduction by splitting at the first `### ` would hand the
/// `Excluded` and `Unchanged` prose the same power the introduction had.
/// `Unchanged` entries reach register coverage through `unchanged_ids`, which
/// accepts only the bullet form.
pub(super) fn trace_rows(trace: &str) -> String {
    ["Changes", "Consumes"]
        .iter()
        .filter_map(|heading| harvest_body(trace, heading))
        .collect()
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

/// Check whether an exact register identifier or supported wildcard covers an ID.
pub(super) fn reference_covers(identifier: &str, references: &str) -> bool {
    references
        .split_whitespace()
        .map(|reference| reference.trim_matches(['|', ',', ';', '`', '(', ')']))
        .any(|reference| {
            reference == identifier
                || reference
                    .strip_suffix("/*")
                    .is_some_and(|prefix| identifier.starts_with(prefix))
                || reference.strip_suffix("@*").is_some_and(|prefix| {
                    identifier
                        .strip_prefix(prefix)
                        .is_some_and(|suffix| suffix.starts_with('@'))
                })
        })
}

/// Collect finding bullets whose ownership must be recorded.
pub(super) fn finding_entries(findings: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut current = None::<String>;
    for line in findings.lines() {
        if is_finding_start(line) {
            finish_finding(&mut current, &mut entries);
            current = Some(line.to_owned());
        } else if is_finding_boundary(line) {
            finish_finding(&mut current, &mut entries);
        } else if let Some(entry) = current.as_mut() {
            entry.push('\n');
            entry.push_str(line);
        }
    }
    finish_finding(&mut current, &mut entries);
    entries
}

/// Identify a heading that starts an owned finding.
fn is_finding_start(line: &str) -> bool {
    ["- H1:", "- H2:", "- H3:", "- H4:", "- OBS-"]
        .iter()
        .any(|prefix| line.trim_start().starts_with(prefix))
}

/// Identify a list item or any Markdown heading that ends a finding entry.
///
/// Matching only `## ` would let a `### ` subheading and its prose be appended
/// to the preceding finding, and `validate_findings` reads task identifiers
/// from the whole entry — so a subheading naming a task would satisfy the
/// owner check for a finding that names none. Any heading level therefore
/// ends the entry.
fn is_finding_boundary(line: &str) -> bool {
    let content = line.trim_start();
    content.starts_with("- ") || content.starts_with('#')
}

/// Move a completed finding into the result list.
fn finish_finding(current: &mut Option<String>, entries: &mut Vec<String>) {
    if let Some(entry) = current.take() {
        entries.push(entry);
    }
}
