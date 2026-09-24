//! Parsers and checks backing the RFC 0006 child-RFC coverage contract.
//!
//! RFC 0006 is a survey RFC: it enumerates every ansible-core candidate it
//! considered and records a disposition for each. This module tree turns that
//! survey, the roadmap, and the child RFCs into a single machine-checkable
//! bijection, so a helper cannot be dropped, double-assigned, or reintroduced
//! under a rejected spelling without a test naming the file and line.
//!
//! Everything here is derived. The accepted set, the deny set, the expected
//! owners, and the clause list are all read out of tracked Markdown. What cannot
//! be derived is transcribed, and each transcription is witnessed against the
//! document it came from: the section 7 inventories and section 6.1's purity
//! aggregate in `survey`, and the ownership grammar of the coverage map's `Owns`
//! column in `map`. RFC 0006 states the former in prose rather than in a table,
//! and the latter is a mini-language no row spells out.
//!
//! The parser contract is deliberately narrow: it reads Markdown table rows
//! inside a named section and never headings. Heading anchoring was rejected
//! because three headings under RFC 0006 section 8 are prose rather than
//! helpers, three name two helpers each, and `basename` and `dirname` have no
//! heading at all. Names are matched by whole-token equality, never substring,
//! because the vocabulary contains `abs` against `is_abs`, `quote` against
//! `shell_quote`, `hash` against `text_hash`, and `subset` against
//! `issubset`.
//!
//! Fenced code blocks are opaque to every scan. A child RFC's section 5 carries
//! example blocks — Jinja snippets, diagnostic text, shell — and a `#`, a `|`,
//! or a backticked name inside one is example content rather than structure.

mod assertions;
mod checks;
mod clauses;
mod inventory;
mod links;
mod map;
mod markdown;
mod registries;
mod roadmap;
mod section7;
mod section8;
mod survey;
mod totals;

pub use checks::{
    coverage_map_status_is_reported, every_accepted_helper_has_exactly_one_owner,
    every_capability_has_a_roadmap_task, every_child_discharges_every_clause,
    inter_document_links_resolve, no_forbidden_helper_is_registered,
    totals_and_purity_aggregate_agree,
};
// Private, but reachable as `super::…` from every child module, which is how
// `section8` gets at `Fences` and `heading_depth`.
use markdown::{Fences, heading_depth, is_separator, row_cells, table_heading};

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Repository-relative path of RFC 0006, the survey RFC.
pub(super) const RFC_0006: &str = "docs/rfcs/0006-ansible-inspired-template-standard-library.md";

/// Repository-relative path of the delivery roadmap.
pub(super) const ROADMAP: &str = "docs/roadmap.md";

/// Repository-relative directory holding the RFC corpus.
pub(super) const RFC_DIR: &str = "docs/rfcs";

/// Jinja namespace a helper occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Namespace {
    /// Invoked as a filter on a piped value.
    Filter,
    /// Invoked as a test after `is`.
    Test,
    /// Invoked as a bare function call.
    Function,
}

impl Namespace {
    /// The lowercase label RFC 0006's tables spell the namespace with.
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Filter => "filter",
            Self::Test => "test",
            Self::Function => "function",
        }
    }
}

/// Whether a registry row introduces a helper or adds an option to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Registration {
    /// One of the 57 new helpers.
    New,
    /// One of the 3 existing helpers gaining an option.
    OptionAdded,
}

impl Registration {
    /// The lowercase label a child RFC's registry spells the kind with.
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::OptionAdded => "option added",
        }
    }
}

/// A surveyed name's disposition, derived from RFC 0006 section 7.
///
/// Rejection is one variant, not three. RFC 0006 table 11 splits the 50 reject
/// rows into 22 already-provides, 10 redundant-alias, and 18 on-principle, but
/// the document states no rule assigning a row to a class and no rule fitted to
/// the tables recovers the split, so a three-way rejection type here would be
/// undecidable at runtime. Under decision `D10` the deny set is the complement
/// of the accepted set and needs no classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Disposition {
    /// Accepted, with the owning section 8 subsection.
    Accept,
    /// Deferred by section 9.
    Defer,
    /// Rejected by section 10, for any of the three reasons table 11 counts.
    Reject,
}

/// One accepted helper, as the document that established it records it.
///
/// The row deliberately carries no source location: every parser that builds one
/// already reports `file:line` in the message that rejects a bad row, and no
/// later check has a use for the provenance.
#[derive(Debug, Clone)]
pub(super) struct Row {
    /// Registered helper name, backticks stripped.
    name: String,
    /// Namespace the helper occupies.
    namespace: Namespace,
}

/// A capability-scoped view of the repository working tree.
///
/// The handle is rooted at the package directory Cargo built this test from,
/// so every path in this module is repository-relative and independent of the
/// working directory the test happens to be run from.
pub(super) struct Repo {
    /// Directory handle rooted at the package root.
    dir: Dir,
}

impl Repo {
    /// Open the package root named by Cargo at build time.
    pub(super) fn open() -> Result<Self> {
        let root = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
        let dir = Dir::open_ambient_dir(root, ambient_authority())
            .with_context(|| format!("open repository root {root}"))?;
        Ok(Self { dir })
    }

    /// Read a repository-relative file as UTF-8.
    fn read(&self, path: &str) -> Result<String> {
        self.dir
            .read_to_string(path)
            .with_context(|| format!("read {path}"))
    }

    /// Whether a repository-relative path exists.
    fn exists(&self, path: &str) -> Result<bool> {
        self.dir
            .try_exists(path)
            .with_context(|| format!("test existence of {path}"))
    }

    /// Repository-relative paths of the `.md` files directly under `dir`.
    ///
    /// Entries are sorted, so a parse over the corpus is deterministic and a
    /// failure message names a stable file.
    fn markdown_files(&self, dir: &str) -> Result<Vec<String>> {
        let mut found = Vec::new();
        let entries = self
            .dir
            .read_dir(dir)
            .with_context(|| format!("list {dir}"))?;
        for candidate in entries {
            let entry = candidate.with_context(|| format!("read a {dir} directory entry"))?;
            let name = entry
                .file_name()
                .with_context(|| format!("read a {dir} entry name"))?;
            let path = Utf8Path::new(&name);
            if path.extension() == Some("md") {
                found.push(format!("{dir}/{name}"));
            }
        }
        found.sort_unstable();
        Ok(found)
    }
}

/// A contiguous slice of a Markdown document, with absolute line numbers.
///
/// Line numbers are preserved through subscripting so that a parse of a nested
/// subsection can still report where in the file a row came from.
pub(super) struct Section<'a> {
    /// One-indexed line number of the first element of `lines`.
    first_line: usize,
    /// The section's lines, with line terminators removed.
    lines: Vec<&'a str>,
}

impl<'a> Section<'a> {
    /// Treat a whole document as one section.
    ///
    /// The document's own path is not retained: every caller holds the `&str`
    /// it read the text from, and takes its diagnostics from that. Carrying a
    /// copy here would be state nothing reads.
    fn whole(text: &'a str) -> Self {
        Self {
            first_line: 1,
            lines: text.lines().collect(),
        }
    }

    /// The subsection headed by `heading`, running to the next heading of the
    /// same or a shallower depth.
    ///
    /// The heading line is part of the returned section. A table placed directly
    /// under a subsection heading is introduced by it and by nothing else, so a
    /// subsection that withheld its own heading would leave that table
    /// attribute-less, and [`Section::tables`] would name it `""` rather than the
    /// heading a caller filters on.
    ///
    /// The heading must match on its full text, so `14.1. Slice` cannot
    /// accidentally select `14.10. Slice`.
    ///
    /// Note that `heading` is matched verbatim, so a caller whose heading also
    /// appears inside a fenced block still lands on the real one: the fences only
    /// blind the *end* scan, leaving the start anchored where it always was.
    fn subsection(&self, heading: &str) -> Option<Self> {
        let start = self.lines.iter().position(|line| line.trim() == heading)?;
        let depth = heading_depth(self.lines.get(start)?)?;
        let rest = self.lines.get(start..)?;
        let end = rest
            .get(1..)?
            .iter()
            .scan(Fences::default(), |fences, line| {
                let fenced = fences.mark(line);
                Some((fenced, line))
            })
            .position(|(fenced, line)| !fenced && heading_depth(line).is_some_and(|d| d <= depth))
            .map_or(rest.len(), |offset| offset + 1);
        Some(Self {
            first_line: self.first_line + start,
            lines: rest.get(..end)?.to_vec(),
        })
    }

    /// This section's `###` subsections, in document order.
    ///
    /// A subsection runs from its own heading to the next heading of the same or
    /// a shallower depth, which is [`Section::subsection`]'s rule applied to
    /// every heading at once. Only depth three is collected, because the sections
    /// that carry subsections in this corpus are depth two: a `####` inside one is
    /// body content, and treating it as a sibling would split its parent.
    ///
    /// The body is fence-free in the same sense [`Section::tables`] is. A child
    /// RFC's clause subsections quote Jinja, shell, and diagnostic text, and a
    /// `#` line inside one of those snippets is a comment rather than a heading;
    /// reading it as one would truncate the subsection and drop whatever
    /// followed. Heading lines are not themselves body, so a `####` inside a
    /// subsection contributes its prose but not its own line.
    fn subsections(&self) -> Vec<Subsection> {
        let headings = self.headings();
        let mut found = Vec::new();
        for (index, (offset, line)) in headings.iter().enumerate() {
            let end = headings
                .get(index + 1)
                .map_or(self.lines.len(), |(next, _)| *next);
            found.push(Subsection {
                heading: line.clone(),
                line: self.first_line + offset,
                body: self.unfenced_body(offset + 1, end),
            });
        }
        found
    }

    /// Every unfenced `###` heading, as (offset, text).
    fn headings(&self) -> Vec<(usize, String)> {
        let mut fences = Fences::default();
        let mut found = Vec::new();
        for (offset, line) in self.lines.iter().enumerate() {
            if fences.mark(line) {
                continue;
            }
            if heading_depth(line) == Some(3)
                && let Some(text) = table_heading(line)
            {
                found.push((offset, text));
            }
        }
        found
    }

    /// The unfenced lines in `start..end`, with trailing whitespace removed.
    ///
    /// The fence scan starts at the section's first line rather than at `start`,
    /// so a body that begins inside a block its own heading opened is read as
    /// fenced. That cannot happen for a heading the scan reached unfenced, but it
    /// costs nothing to be right about.
    fn unfenced_body(&self, start: usize, end: usize) -> Vec<String> {
        let mut fences = Fences::default();
        let mut body = Vec::new();
        for (offset, line) in self.lines.iter().enumerate().take(end) {
            if fences.mark(line) {
                continue;
            }
            if offset >= start {
                body.push(line.trim_end().to_owned());
            }
        }
        body
    }

    /// Every Markdown table in this section, in document order, paired with the
    /// heading text that precedes it.
    ///
    /// The header row is dropped and the `|---|` separator skipped, so each
    /// returned row is a data row. A table ends at the first line that is not a
    /// table row, which is what keeps two adjacent subtables distinct.
    ///
    /// Fenced lines are skipped in both roles. A `|` line inside a fence is
    /// example content, and a `#` line inside a fence is a comment — in a Jinja
    /// or shell snippet, both are ordinary text and neither is structure.
    fn tables(&self) -> Vec<(String, Vec<RawRow>)> {
        let mut tables: Vec<(String, Vec<RawRow>)> = Vec::new();
        let mut fences = Fences::default();
        let mut heading = String::new();
        let mut body = false;
        for (offset, line) in self.lines.iter().enumerate() {
            if fences.mark(line) {
                body = false;
                continue;
            }
            let number = self.first_line + offset;
            if let Some(text) = table_heading(line) {
                heading = text;
                body = false;
                continue;
            }
            let Some(cells) = row_cells(line) else {
                body = false;
                continue;
            };
            if is_separator(&cells) {
                continue;
            }
            if !body {
                // The first row of a table is its header, and the separator
                // under it is skipped, so the rows that follow are data rows.
                tables.push((heading.clone(), Vec::new()));
                body = true;
                continue;
            }
            if let Some((_, rows)) = tables.last_mut() {
                rows.push(RawRow {
                    cells,
                    line: number,
                });
            }
        }
        tables
            .into_iter()
            .filter(|(_, rows)| !rows.is_empty())
            .collect()
    }
}

/// One `###` subsection of a section, with its body.
pub(super) struct Subsection {
    /// The heading text, without its hashes.
    pub(super) heading: String,
    /// One-indexed line number of the heading.
    pub(super) line: usize,
    /// The subsection's own lines, fences and heading excluded.
    pub(super) body: Vec<String>,
}

/// One Markdown table row, with the line it was read from.
pub(super) struct RawRow {
    /// Cell text, trimmed, in column order.
    cells: Vec<String>,
    /// One-indexed line number.
    line: usize,
}

impl RawRow {
    /// The cell at `column`, or an error naming this row when the table is
    /// narrower than the caller expects.
    fn cell(&self, column: usize, what: &str) -> Result<&str> {
        self.cells
            .get(column)
            .map(String::as_str)
            .with_context(|| format!("row at line {} has no {what} column", self.line))
    }
}

/// Strip one surrounding pair of backticks and any surrounding whitespace.
///
/// A cell may hold several backticked names separated by ` / `; callers that
/// expect that split on [`NAME_SEPARATOR`] first.
pub(super) fn strip_backticks(cell: &str) -> &str {
    let trimmed = cell.trim();
    trimmed
        .strip_prefix('`')
        .and_then(|rest| rest.strip_suffix('`'))
        .unwrap_or(trimmed)
}

/// The separator RFC 0006 section 7.8 uses between alias-group names.
pub(super) const NAME_SEPARATOR: &str = " / ";

/// Expand a name cell into its individual backtick-stripped names.
///
/// Section 7.8 records that rows cover alias groups as single entries, so
/// `` `d` / `default` `` contributes two names.
pub(super) fn names_in(cell: &str) -> Vec<String> {
    if cell.trim() == "—" || cell.trim() == "-" {
        return Vec::new();
    }
    cell.split(NAME_SEPARATOR)
        .map(|name| strip_backticks(name).to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

/// Every backticked span in `text`, in order.
pub(super) fn backticked(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .map(ToOwned::to_owned)
        .collect()
}

/// A sorted, de-duplicated view of `names`, for diffing in assertions.
pub(super) fn name_set<'a>(names: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
    names.into_iter().map(ToOwned::to_owned).collect()
}
