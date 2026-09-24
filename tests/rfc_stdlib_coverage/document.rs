//! The document layer: sections, subsections, and raw table rows.
//!
//! Every parser in this module tree reads its source through this layer, so the
//! question "where does this section end" is answered once. The lexical
//! judgements it rests on — is a line a heading, is it a table row, is it inside
//! a fence — belong to `super::markdown`; what lives here is the structure built
//! on top of them.
//!
//! Line numbers are absolute and preserved through subscripting, so a parse of a
//! nested subsection still reports where in the file a row came from.
//!
//! Split out of `mod.rs` to keep that module under Whitaker's 400-line
//! `module_max_lines` ceiling, not as a new resolution boundary: `mod.rs` owns
//! this module and re-exports it, so each item reaches a sibling exactly as it
//! did before the split, and nothing outside this module tree may use it.

use anyhow::{Context, Result};

use super::{Fences, heading_depth, is_separator, row_cells, table_heading};

/// A contiguous slice of a Markdown document, with absolute line numbers.
///
/// Line numbers are preserved through subscripting so that a parse of a nested
/// subsection can still report where in the file a row came from.
pub(super) struct Section<'a> {
    /// One-indexed line number of the first element of `lines`.
    first_line: usize,
    /// The section's lines, with line terminators removed.
    ///
    /// Public to the module tree because three parsers scan the raw lines
    /// themselves rather than through a method here: `section8` re-derives an
    /// end offset for a `###` heading whose text is not known in advance,
    /// `roadmap` walks `###` step headings, and `clauses` reads clause ids off
    /// them. Each is a different question from the ones [`Section::tables`] and
    /// [`Section::subsections`] answer, so an accessor for any one of them would
    /// be unused by the other two.
    pub(super) lines: Vec<&'a str>,
}

impl<'a> Section<'a> {
    /// Treat a whole document as one section.
    ///
    /// The document's own path is not retained: every caller holds the `&str`
    /// it read the text from, and takes its diagnostics from that. Carrying a
    /// copy here would be state nothing reads.
    pub(super) fn whole(text: &'a str) -> Self {
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
    pub(super) fn subsection(&self, heading: &str) -> Option<Self> {
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
    pub(super) fn subsections(&self) -> Vec<Subsection> {
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
    pub(super) fn tables(&self) -> Vec<(String, Vec<RawRow>)> {
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
    pub(super) line: usize,
}

impl RawRow {
    /// The cell at `column`, or an error naming this row when the table is
    /// narrower than the caller expects.
    pub(super) fn cell(&self, column: usize, what: &str) -> Result<&str> {
        self.cells
            .get(column)
            .map(String::as_str)
            .with_context(|| format!("row at line {} has no {what} column", self.line))
    }
}

