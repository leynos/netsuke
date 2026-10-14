//! Loads fenced examples from project Markdown for executable tests.
//!
//! Each fence in `README.md`, `docs/users-guide.md`, and the template
//! standard-library guide must be preceded by a `tested-example` marker. The
//! developers' guide holds many illustrative fences, so only its marked fences
//! are loaded: the Rust API snippets that mirror executable doctests.
//! Integration and behavioural tests share this module so they exercise the
//! published text rather than copied fixtures.

use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use std::collections::HashSet;
use tempfile::{TempDir, tempdir};
use test_support::fs as test_fs;
use test_support::netsuke::NetsukeRun;

const DOCUMENTS: &[(&str, FencePolicy)] = &[
    ("README.md", FencePolicy::RequireMarkers),
    ("docs/users-guide.md", FencePolicy::RequireMarkers),
    (
        "docs/stdlib-yaml-and-jinja-guide.md",
        FencePolicy::RequireMarkers,
    ),
    ("docs/developers-guide.md", FencePolicy::MarkedOnly),
];
const MARKER_PREFIX: &str = "<!-- tested-example: ";
const MARKER_SUFFIX: &str = " -->";
static EMPTY_MARKER: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| format!("{MARKER_PREFIX}{}", MARKER_SUFFIX.trim_start()));

/// How a document treats a fence without a `tested-example` marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FencePolicy {
    /// Reject an unmarked fence: every example in the document is tested.
    RequireMarkers,
    /// Skip an unmarked fence and its body: only marked fences are tested.
    MarkedOnly,
}

#[derive(Clone, Copy)]
struct Cursor {
    source: &'static str,
    line_index: usize,
}

impl Cursor {
    fn error(self, message: &str) -> String {
        format!("{}:{} {message}", self.source, self.line_index + 1)
    }
}

/// One marked fenced example loaded from a user-facing document.
#[derive(Debug, Eq, PartialEq)]
pub struct DocumentedExample {
    /// Stable identifier declared by the `tested-example` marker.
    pub id: String,
    /// Markdown fence language.
    pub language: String,
    /// Exact text inside the fence, including a trailing newline.
    pub body: String,
}

/// Load every marked example and reject unmarked or duplicate fences.
///
/// # Errors
///
/// Returns an error when a document cannot be read, a marker is malformed,
/// a fence is unmarked or unterminated, or an identifier is duplicated.
pub fn load_documented_examples() -> Result<Vec<DocumentedExample>> {
    let mut examples = Vec::new();
    for &(path, policy) in DOCUMENTS {
        examples.extend(load_document(path, policy)?);
    }

    let mut ids = HashSet::new();
    for example in &examples {
        ensure!(
            ids.insert(example.id.as_str()),
            "duplicate tested-example identifier '{}'",
            example.id
        );
    }
    Ok(examples)
}

/// Load the documented example identified by `id`.
///
/// # Errors
///
/// Returns an error when the documents are invalid or `id` is absent.
pub fn documented_example(id: &str) -> Result<DocumentedExample> {
    load_documented_examples()?
        .into_iter()
        .find(|example| example.id == id)
        .with_context(|| format!("documented example '{id}' should exist"))
}

/// Create an isolated workspace whose `Netsukefile` is a documented example.
///
/// # Errors
///
/// Returns an error when the example cannot be loaded or written.
pub fn manifest_workspace(id: &str) -> Result<TempDir> {
    let example = documented_example(id)?;
    ensure!(
        example.language == "yaml",
        "documented example '{id}' should be YAML, got '{}'",
        example.language
    );
    let workspace = tempdir().with_context(|| format!("create workspace for '{id}'"))?;
    test_fs::write(workspace.path().join("Netsukefile"), example.body)
        .with_context(|| format!("write Netsukefile for '{id}'"))?;
    Ok(workspace)
}

/// Assert that a documented Netsuke invocation completed successfully.
///
/// # Errors
///
/// Returns an error containing the captured output when the invocation fails.
pub fn assert_success(run: &NetsukeRun, context: &str) -> Result<()> {
    ensure!(
        run.success,
        "{context} should succeed; stdout:\n{}\nstderr:\n{}",
        run.stdout,
        run.stderr
    );
    Ok(())
}

fn load_document(path: &'static str, policy: FencePolicy) -> Result<Vec<DocumentedExample>> {
    let repository_root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let contents = test_fs::read_to_string(repository_root.join(path))
        .with_context(|| format!("read {path}"))?;
    parse_document_with_policy(path, &contents, policy)
}

/// Parse a document, treating unmarked fences according to `policy`.
///
/// # Errors
///
/// Returns an error when a marker is malformed, a fence is unterminated, an
/// identifier is duplicated, or `policy` requires a missing marker.
pub(crate) fn parse_document_with_policy(
    source: &'static str,
    contents: &str,
    policy: FencePolicy,
) -> Result<Vec<DocumentedExample>> {
    let mut lines = contents.lines().enumerate();
    let mut examples = Vec::new();
    let mut ids = HashSet::new();

    while let Some((line_index, line)) = lines.next() {
        let cursor = Cursor { source, line_index };
        if let Some(id) = parse_marker(line) {
            ensure!(
                !id.trim().is_empty(),
                "{}",
                cursor.error("tested-example identifier must not be empty")
            );
            ensure!(ids.insert(id), "duplicate tested-example identifier '{id}'");
            examples.push(read_marked_example(&cursor, id, &mut lines)?);
        } else if let Some(opening) = FenceOpening::parse(line)
            && policy == FencePolicy::MarkedOnly
        {
            // Consume the unmarked body so a marker-like line inside it is
            // never read as a marker.
            read_fence_body(source, line_index, opening, &mut lines)?;
        } else {
            reject_invalid_example_line(&cursor, line)?;
        }
    }

    Ok(examples)
}

fn reject_invalid_example_line(cursor: &Cursor, line: &str) -> Result<()> {
    ensure!(
        line != EMPTY_MARKER.as_str(),
        "{}",
        cursor.error("tested-example identifier must not be empty")
    );
    reject_unmarked_fence(cursor, line)
}

fn read_marked_example<'a>(
    cursor: &Cursor,
    id: &str,
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
) -> Result<DocumentedExample> {
    let (fence_index, fence) =
        next_non_empty_line(lines).with_context(|| cursor.error("marker has no fence"))?;
    let fence_cursor = Cursor {
        source: cursor.source,
        line_index: fence_index,
    };
    let opening = FenceOpening::parse(fence)
        .with_context(|| fence_cursor.error("expected an opening fence after marker"))?;
    let language = opening.info;
    ensure!(
        !language.is_empty(),
        "{}",
        fence_cursor.error("fence should declare a language")
    );
    let body = read_fence_body(cursor.source, fence_index, opening, lines)?;
    Ok(DocumentedExample {
        id: id.to_owned(),
        language: language.to_owned(),
        body,
    })
}

fn reject_unmarked_fence(cursor: &Cursor, line: &str) -> Result<()> {
    ensure!(
        FenceOpening::parse(line).is_none(),
        "{}",
        cursor.error("fence is missing a tested-example marker")
    );
    Ok(())
}

fn parse_marker(line: &str) -> Option<&str> {
    line.strip_prefix(MARKER_PREFIX)
        .and_then(|value| value.strip_suffix(MARKER_SUFFIX))
}

fn next_non_empty_line<'a>(
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
) -> Option<(usize, &'a str)> {
    lines.find(|(_, line)| !line.is_empty())
}

/// The opening line of a fenced code block.
///
/// A fence is a run of at least three backticks or tildes at column 0, and it
/// closes only on a line of the same character at least as long, so a longer
/// fence can quote a shorter one.
#[derive(Clone, Copy)]
struct FenceOpening<'a> {
    delimiter: char,
    run_length: usize,
    info: &'a str,
}

impl<'a> FenceOpening<'a> {
    /// Recognize `line` as a fence opening, capturing its delimiter and run.
    fn parse(line: &'a str) -> Option<Self> {
        let delimiter = line.chars().next().filter(|ch| matches!(ch, '`' | '~'))?;
        let info = line.trim_start_matches(delimiter);
        // Both delimiters are one byte, so the byte difference is the run.
        let run_length = line.len() - info.len();
        (run_length >= 3).then_some(Self {
            delimiter,
            run_length,
            info,
        })
    }

    /// Report whether `line` closes this fence.
    fn is_closed_by(self, line: &str) -> bool {
        let candidate = line.trim_end();
        candidate.len() >= self.run_length && candidate.chars().all(|ch| ch == self.delimiter)
    }
}

fn read_fence_body<'a>(
    source: &str,
    fence_index: usize,
    opening: FenceOpening<'_>,
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
) -> Result<String> {
    let mut body = String::new();
    for (_, line) in lines {
        if opening.is_closed_by(line) {
            return Ok(body);
        }
        body.push_str(line);
        body.push('\n');
    }
    anyhow::bail!("{source}:{} fence is not terminated", fence_index + 1)
}
