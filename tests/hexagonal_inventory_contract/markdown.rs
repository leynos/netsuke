//! Parse the Markdown structures used by the inventory integrity contract.

use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;

/// Select a heading depth supported by the inventory contract parser.
#[derive(Clone, Copy)]
pub(super) enum HeadingLevel {
    /// Match a second-level Markdown heading.
    Section,
    /// Match a third-level Markdown heading.
    Subsection,
}

/// Extract a heading's body up to the next heading at the same level.
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
        let title = line
            .trim_end_matches(['\r', '\n'])
            .strip_prefix(marker)
            .map(strip_heading_number);

        if body_start.is_some() && title.is_some() {
            return text.get(body_start?..offset);
        }
        if title == Some(heading) {
            body_start = Some(offset + line.len());
        }
        offset += line.len();
    }

    body_start.and_then(|start| text.get(start..))
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

/// Check whether a register identifier follows the area and site grammar.
pub(super) fn is_well_formed_register_id(identifier: &str) -> bool {
    let Some((area, body)) = identifier.split_once('/') else {
        return false;
    };
    has_valid_area_and_body(area, body) && has_valid_register_body(body)
}

/// Check whether the register body uses either a type or a site identifier.
fn has_valid_register_body(body: &str) -> bool {
    let Some((item, site)) = body.split_once('@') else {
        return !body.contains('#');
    };
    has_valid_item(item) && has_valid_site(site)
}

/// Check whether the item before the site marker is non-empty and unambiguous.
fn has_valid_item(item: &str) -> bool {
    !item.is_empty() && !item.contains('#')
}

/// Check whether a site has a symbol and a supported kind.
fn has_valid_site(site: &str) -> bool {
    let Some((symbol, kind)) = site.split_once('#') else {
        return false;
    };
    !symbol.is_empty()
        && !has_site_delimiter(symbol)
        && !has_site_delimiter(kind)
        && has_supported_site_kind(kind)
}

/// Reject separators that would make a site split ambiguous.
fn has_site_delimiter(value: &str) -> bool {
    value
        .chars()
        .any(|character| matches!(character, '@' | '#'))
}

/// Check the closed set of site kinds and their numbered variants.
fn has_supported_site_kind(kind: &str) -> bool {
    ["constructor", "mutator", "consumer", "api-reachable"]
        .iter()
        .any(|known_kind| matches_site_kind(kind, known_kind))
}

/// Match a site kind or its numbered suffix.
fn matches_site_kind(kind: &str, known_kind: &str) -> bool {
    kind == known_kind
        || kind
            .strip_prefix(known_kind)
            .and_then(|suffix| suffix.strip_prefix('-'))
            .is_some_and(is_ascii_number)
}

/// Check that a numbered site-kind suffix contains only decimal digits.
fn is_ascii_number(number: &str) -> bool {
    !number.is_empty() && number.chars().all(|character| character.is_ascii_digit())
}

/// Check the register area's closed set and its non-empty, whitespace-free body.
fn has_valid_area_and_body(area: &str, body: &str) -> bool {
    matches!(area, "A1" | "A2" | "A3" | "A4" | "A5" | "A6" | "A7")
        && !body.is_empty()
        && !body.chars().any(char::is_whitespace)
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
    let Some(excluded) = heading_body(trace, "Excluded", HeadingLevel::Subsection) else {
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
        .filter_map(|heading| heading_body(trace, heading, HeadingLevel::Subsection))
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
    let Some(unchanged) = heading_body(trace, "Unchanged", HeadingLevel::Subsection) else {
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

/// Identify a list item or section heading that ends a finding entry.
fn is_finding_boundary(line: &str) -> bool {
    let content = line.trim_start();
    content.starts_with("- ") || content.starts_with("## ")
}

/// Move a completed finding into the result list.
fn finish_finding(current: &mut Option<String>, entries: &mut Vec<String>) {
    if let Some(entry) = current.take() {
        entries.push(entry);
    }
}
