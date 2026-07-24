//! Loads fenced examples from user-facing Markdown for executable tests.
//!
//! Each fence in `README.md` and `docs/users-guide.md` must be preceded by a
//! `tested-example` marker. Integration and behavioural tests share this
//! module so they exercise the published text rather than copied fixtures.

use anyhow::{Context, Result, ensure};
use std::collections::HashSet;
use std::path::Path;
use tempfile::{TempDir, tempdir};
use test_support::fs as test_fs;
use test_support::netsuke::NetsukeRun;

const DOCUMENT_PATHS: &[&str] = &["README.md", "docs/users-guide.md"];
const MARKER_PREFIX: &str = "<!-- tested-example: ";
const MARKER_SUFFIX: &str = " -->";
static EMPTY_MARKER: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| format!("{MARKER_PREFIX}{}", MARKER_SUFFIX.trim_start()));

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
    for path in DOCUMENT_PATHS {
        examples.extend(load_document(path)?);
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

fn load_document(path: &'static str) -> Result<Vec<DocumentedExample>> {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let contents = test_fs::read_to_string(repository_root.join(path))
        .with_context(|| format!("read {path}"))?;
    parse_document(path, &contents)
}

pub(crate) fn parse_document(
    source: &'static str,
    contents: &str,
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
    let language = fence
        .strip_prefix("```")
        .with_context(|| fence_cursor.error("expected an opening fence after marker"))?;
    ensure!(
        !language.is_empty(),
        "{}",
        fence_cursor.error("fence should declare a language")
    );
    let body = read_fence_body(cursor.source, fence_index, lines)?;
    Ok(DocumentedExample {
        id: id.to_owned(),
        language: language.to_owned(),
        body,
    })
}

fn reject_unmarked_fence(cursor: &Cursor, line: &str) -> Result<()> {
    ensure!(
        !line.starts_with("```"),
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

fn read_fence_body<'a>(
    source: &str,
    fence_index: usize,
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
) -> Result<String> {
    let mut body = String::new();
    for (_, line) in lines {
        if line == "```" {
            return Ok(body);
        }
        body.push_str(line);
        body.push('\n');
    }
    anyhow::bail!("{source}:{} fence is not terminated", fence_index + 1)
}
