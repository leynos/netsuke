//! Parse the Markdown structures used by the inventory integrity contract.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;

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
pub(super) fn is_heading(line: &str) -> bool {
    let depth = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    (1..=6).contains(&depth) && line.chars().nth(depth) == Some(' ')
}

/// Report whether a line is a Markdown heading at or above `level`.
pub(super) fn is_heading_at_or_above(line: &str, level: HeadingLevel) -> bool {
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

/// The register subsection whose table classifies oracles rather than items.
///
/// Its rows name an item and a non-production context; they never register an
/// identifier, so it is the one place in the register where a table header
/// other than `ID` is correct.
const NON_PRODUCTION_APPENDIX: &str = "Non-production site appendix";

/// Return the headers of register tables that do not open with `ID`.
///
/// `register_ids` skips every row whose table header it does not recognise,
/// which is the right reading rule for a parser but the wrong contract for the
/// register: a table renamed from `ID` to anything else would drop its
/// identifiers from the uniqueness, well-formedness, and coverage checks and
/// leave them silently untraced. `validate_populated` cannot close that hole,
/// because it only requires the section to supply at least one identifier and
/// one surviving table is enough. Reporting the header is what turns the skip
/// into a failure; a table with no header at all reports its first row.
pub(super) fn non_id_register_headers(register: &str) -> Vec<String> {
    let mut scan = RegisterTableScan::default();
    for line in register.lines() {
        scan.visit(line);
    }
    scan.headers
}

/// Track the register scan's position across lines and collect bad headers.
///
/// The position is stateful — whether the next row opens a table depends on
/// the line before it — so it belongs in one place rather than in three local
/// variables threaded through a nested loop.
#[derive(Default)]
struct RegisterTableScan {
    headers: Vec<String>,
    in_appendix: bool,
    table_open: bool,
}

impl RegisterTableScan {
    /// Update the scan position from one line of the register.
    fn visit(&mut self, line: &str) {
        if let Some(title) = subsection_title(line) {
            self.enter_subsection(title);
        } else if let Some(cell) = first_cell(line) {
            self.enter_table_row(cell);
        } else {
            self.table_open = false;
        }
    }

    /// Restart table tracking inside a newly entered register subsection.
    fn enter_subsection(&mut self, title: &str) {
        self.in_appendix = title == NON_PRODUCTION_APPENDIX;
        self.table_open = false;
    }

    /// Record a misheaded header, then mark the table as open.
    fn enter_table_row(&mut self, cell: &str) {
        if is_misheaded_register_table(self.table_open, self.in_appendix, cell) {
            self.headers.push(cell.to_owned());
        }
        self.table_open = true;
    }
}

/// Report whether a table row opens a register table that is not `ID`-headed.
///
/// Only the row that opens a table can be a header, so a later row's text is
/// never a header however it reads, and the appendix classifies oracles rather
/// than registering items.
fn is_misheaded_register_table(table_open: bool, in_appendix: bool, cell: &str) -> bool {
    !table_open && !in_appendix && cell != "ID"
}

/// Return the title of a third-level heading line, if the line is one.
fn subsection_title(line: &str) -> Option<&str> {
    let title = line.trim_end_matches(['\r', '\n']).strip_prefix("### ")?;
    Some(strip_heading_number(title))
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
pub(super) fn first_cell(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix('|')?;
    let (cell, _) = rest.split_once('|')?;
    Some(cell.trim())
}

/// Identify a Markdown table's delimiter cell.
pub(super) fn is_table_separator(cell: &str) -> bool {
    !cell.is_empty()
        && cell
            .chars()
            .all(|character| matches!(character, '-' | ':' | ' '))
}

/// Collect one entry per bullet the predicate opens.
///
/// An indented line continues the open entry and any unindented line closes
/// it, so the wrapped bullet and the trailing prose both land where a reader
/// expects: continuation inside the entry, prose outside every entry. That
/// distinction is why a schema requiring a fact per bullet cannot collect its
/// identifiers across the section instead — one entry's fact would satisfy its
/// neighbour's omission.
///
/// A blank line does not by itself close the open entry. Markdown allows a
/// list item to hold several paragraphs, so a blank line followed by an
/// indented line is still that entry — a continuation whose owner, fact, or
/// commit is written one line below a gap. Only the line that follows the gap
/// decides: indented continues, anything else closes and, if it opens a bullet,
/// opens the next entry. Treating the blank line as the close dropped the
/// indented line silently, and with it whatever fact the entry was written to
/// carry.
pub(super) fn bullet_entries(text: &str, opens: impl Fn(&str) -> bool) -> Vec<String> {
    let mut entries = Vec::new();
    let mut open = None::<String>;
    for line in text.lines() {
        if opens(line) {
            close_entry(&mut open, &mut entries);
            open = Some(line.trim_start().to_owned());
        } else if is_list_continuation(line) {
            extend_entry(&mut open, line);
        } else if !line.trim().is_empty() {
            close_entry(&mut open, &mut entries);
        }
    }
    close_entry(&mut open, &mut entries);
    entries
}

/// Report whether a line is indented far enough to continue the bullet above.
///
/// Only leading whitespace qualifies. Deciding this here rather than inline
/// gives the caller one branch per line, which is what keeps the collector's
/// nesting shallow enough to read at a glance.
fn is_list_continuation(line: &str) -> bool {
    line.starts_with(' ') || line.starts_with('\t')
}

/// Append an indented continuation line to the entry it belongs to.
///
/// A continuation with no open entry belongs to no bullet — it is prose before
/// the list, or an indented block under an earlier heading — and must be
/// dropped rather than started as an entry, or it would be read as one.
fn extend_entry(open: &mut Option<String>, line: &str) {
    if let Some(entry) = open.as_mut() {
        entry.push('\n');
        entry.push_str(line.trim());
    }
}

/// Move the open entry, if any, into the result list and clear it.
///
/// Taking the value rather than copying it means an entry is pushed exactly
/// once however many times a boundary is met.
fn close_entry(open: &mut Option<String>, entries: &mut Vec<String>) {
    entries.extend(open.take());
}

/// Collect finding bullets whose ownership must be recorded.
///
/// The entry boundary is `bullet_entries`', shared with the obligation and
/// landed-work readers: an indented line continues the open entry and any
/// unindented line closes it. That last part is the whole point. A paragraph
/// separated from its bullet by a blank line is unindented, and Markdown ends
/// the list item there, so `validate_findings` must not read an owner out of
/// it. The earlier reader ended an entry only at a list item or a heading, so
/// `- OBS-1: no owner.` followed by prose naming a task satisfied the owner
/// check for a finding that named none. Ending at headings alone would have
/// left that open however many heading levels were enumerated, because the
/// paragraph needs no heading at all.
pub(super) fn finding_entries(findings: &str) -> Vec<String> {
    bullet_entries(findings, is_finding_start)
}

/// Identify a heading that starts an owned finding.
fn is_finding_start(line: &str) -> bool {
    let content = line.trim_start();
    let Some(bullet) = content.strip_prefix("- ") else {
        return false;
    };
    FINDING_LABEL.is_match(bullet)
}

/// The finding label a bullet may open with, however it is decorated.
///
/// A decoration the pattern does not allow would make the bullet look like
/// prose, and `validate_findings` checks only the entries it collects — so
/// `- **OBS-1**: no owner.` would bind nothing and still satisfy the
/// population guard through a neighbour. Matching a short list of readable
/// prefixes is what let a bold label escape, so the pattern allows any
/// non-word characters around the identifier and then reports the bullet as an
/// entry, where the owner check applies to it.
static FINDING_LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\W*(?:H[1-4]|OBS-\d+)\W*:").expect("finding label pattern should compile")
});
