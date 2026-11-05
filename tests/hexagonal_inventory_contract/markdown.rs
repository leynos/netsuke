//! Parse the Markdown structures used by the inventory integrity contract.

use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;

/// Return a second-level section body.
pub(super) fn section<'text>(text: &'text str, heading: &str) -> Option<&'text str> {
    heading_body(text, "## ", heading)
}

/// Return a third-level subsection body.
pub(super) fn subsection<'text>(text: &'text str, heading: &str) -> Option<&'text str> {
    heading_body(text, "### ", heading)
}

/// Extract a heading's body up to the next heading at the same level.
fn heading_body<'text>(text: &'text str, marker: &str, heading: &str) -> Option<&'text str> {
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
pub(super) fn regex_values(pattern: &str, text: &str) -> BTreeSet<String> {
    Regex::new(pattern).map_or_else(
        |_| BTreeSet::new(),
        |regex| {
            regex
                .find_iter(text)
                .map(|matched| matched.as_str().to_owned())
                .collect()
        },
    )
}

/// Return first-column identifiers from the inventory's register tables.
pub(super) fn register_ids(text: &str) -> Vec<String> {
    let Some(register) = section(text, "Register") else {
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
    let Some(excluded) = subsection(trace, "Excluded") else {
        return BTreeMap::new();
    };
    let mut tasks = BTreeMap::new();
    for line in excluded.lines() {
        for task in task_ids(line) {
            tasks.insert(task.clone(), exclusion_reason(line, &task));
        }
    }
    tasks
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
