//! The Markdown lexical layer: headings, fences, and table rows.
//!
//! Every scan in this module tree reads a document line by line and asks the
//! same three questions of each line: is it a heading, is it a table row, and is
//! it inside a fenced code block. Keeping those three answers here means the
//! answers cannot drift between the scans that ask them.
//!
//! Fence tracking is the reason this is a module rather than three functions. A
//! child RFC's section 5 carries example snippets — Jinja, diagnostic text,
//! shell — in which a `#` opens a comment, a `|` separates shell pipeline
//! stages, and a backticked word is prose. A scan that reads any of those as
//! structure corrupts the parse silently, and it corrupts it by *truncating*: a
//! fenced `#` line reads as a depth-1 heading, ends the enclosing subsection, and
//! drops every helper specified below it. The resulting diagnostic blames the
//! helper rather than the code block.

/// The Markdown heading depth of `line`, if it is a heading.
///
/// An ATX heading is one to six hashes followed by a space or tab and then
/// heading text. The separator is required rather than merely assumed, because
/// the RFC corpus cites issues as bare `#596` and a scan that accepted any
/// hash-prefixed line would read a citation at the start of a paragraph as a
/// heading. The damage would be silent: a heading ends the enclosing section,
/// so the citation would truncate the scan and drop every helper below it.
///
/// Seven or more hashes is not a heading of any depth, so the count is bounded
/// above as well.
pub(super) fn heading_depth(line: &str) -> Option<usize> {
    let hashes = line.len() - line.trim_start_matches('#').len();
    let rest = line.get(hashes..)?;
    let separated = rest.starts_with(' ') || rest.starts_with('\t');
    ((1..=6).contains(&hashes) && separated && !rest.trim().is_empty()).then_some(hashes)
}

/// The heading text of `line`, if it is a heading.
pub(super) fn table_heading(line: &str) -> Option<String> {
    heading_depth(line)?;
    Some(line.trim().trim_start_matches('#').trim().to_owned())
}

/// The trimmed cells of `line`, if it is a Markdown table row.
///
/// A line that is only `|` yields no cells rather than an empty slice, so a
/// stray pipe cannot be mistaken for a one-column row.
pub(super) fn row_cells(line: &str) -> Option<Vec<String>> {
    let inner = line.trim().strip_prefix('|')?.strip_suffix('|')?;
    Some(
        inner
            .split('|')
            .map(|cell| cell.trim().to_owned())
            .collect(),
    )
}

/// Whether a row is the `|---|---|` separator under a table header.
pub(super) fn is_separator(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|cell| {
            !cell.is_empty() && cell.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')
        })
}

/// Fenced-code-block state for a line-by-line scan.
///
/// Callers feed lines in order and use [`Fences::mark`]'s return value: a fenced
/// line is opaque, and the fence's own opening and closing lines count as
/// fenced. A scan that skips those lines is a scan that never mistakes an
/// example for the document.
///
/// Only the two Markdown fence styles are tracked. The delimiter is remembered
/// rather than just the open/closed state, so a `~~~` block containing a line of
/// backticks does not close early, and a much longer closing run than the one
/// that opened the block is accepted, as `CommonMark` requires.
#[derive(Default)]
pub(super) struct Fences {
    /// The delimiter that opened the current block, while one is open.
    open: Option<Delimiter>,
}

impl Fences {
    /// Record `line` against the current state.
    ///
    /// Returns `true` when the line is part of a fenced block, including the
    /// line that opens or closes it.
    pub(super) fn mark(&mut self, line: &str) -> bool {
        let Some(delimiter) = Delimiter::opening(line) else {
            return self.open.is_some();
        };
        self.open = match self.open {
            // Anything met inside an open block either closes it or is content:
            // CommonMark nests neither fence style, so a `~~~` line does not
            // open inside a backtick block.
            Some(opened) if delimiter.closes(opened) => None,
            Some(opened) => Some(opened),
            None => Some(delimiter),
        };
        true
    }
}

/// A fence delimiter line, decomposed.
#[derive(Clone, Copy)]
struct Delimiter {
    /// The character the fence is drawn with.
    character: char,
    /// How many of them run together.
    run: usize,
    /// Whether an info string follows the run.
    info: bool,
}

impl Delimiter {
    /// Read `line` as a fence delimiter, if it is one.
    ///
    /// `None` also covers a backtick fence whose info string itself contains a
    /// backtick, which `CommonMark` forbids — that shape is an inline code span
    /// opening a line, not a fence.
    fn opening(line: &str) -> Option<Self> {
        let trimmed = line.trim_start();
        let (character, run) = fence_run(trimmed);
        if run < 3 || !matches!(character, '`' | '~') {
            return None;
        }
        let rest = trimmed.get(run..).unwrap_or("");
        if character == '`' && rest.contains('`') {
            return None;
        }
        Some(Self {
            character,
            run,
            info: !rest.trim().is_empty(),
        })
    }

    /// Whether this delimiter closes a block that `opened` opened.
    const fn closes(self, opened: Self) -> bool {
        self.character == opened.character && self.is_closing_run(opened.run)
    }

    /// Whether the run closes: long enough, and carrying no info string.
    ///
    /// Split from [`Delimiter::closes`] so each predicate holds one conjunction,
    /// which is what keeps the guard in [`Fences::mark`] within the branch limit
    /// Whitaker's `conditional_max_n_branches` sets.
    const fn is_closing_run(self, opened_run: usize) -> bool {
        self.run >= opened_run && !self.info
    }
}

/// The leading run of fence characters in `line`, with its length.
///
/// `line` must already be trimmed of leading whitespace.
fn fence_run(line: &str) -> (char, usize) {
    let character = line.chars().next().unwrap_or(' ');
    (
        character,
        line.chars().take_while(|ch| *ch == character).count(),
    )
}

#[cfg(test)]
mod heading_tests {
    use super::heading_depth;

    /// A citation is not a heading, and the two are one space apart.
    ///
    /// This is the contract that keeps a bare `#596` in prose from ending the
    /// enclosing section: accepting it would truncate the scan and drop every
    /// helper specified below it, and nothing else in the suite would notice,
    /// because the corpus cites issues only inside linked prose today.
    #[test]
    fn citation_is_not_a_heading() {
        assert_eq!(heading_depth("#596 is the originating issue"), None);
        assert_eq!(heading_depth("####596"), None);
        assert_eq!(heading_depth("## comment"), Some(2));
    }

    /// One to six hashes, a separating space or tab, and text is a heading.
    #[test]
    fn separated_hashes_are_headings() {
        assert_eq!(heading_depth("# Title"), Some(1));
        assert_eq!(heading_depth("### 5.1. Registry"), Some(3));
        assert_eq!(heading_depth("###### deep"), Some(6));
        assert_eq!(heading_depth("#\tTabbed"), Some(1));
    }

    /// The forms that carry no heading text, or too many hashes, are not
    /// headings at any depth.
    #[test]
    fn degenerate_runs_are_not_headings() {
        assert_eq!(heading_depth(""), None);
        assert_eq!(heading_depth("#"), None);
        assert_eq!(heading_depth("# "), None);
        assert_eq!(heading_depth("#\t  "), None);
        assert_eq!(heading_depth("####### seven"), None);
        assert_eq!(heading_depth("no leading hash"), None);
        assert_eq!(heading_depth("   # indented above three"), None);
    }
}
