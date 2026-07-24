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

const DOCUMENT_PATHS: &[&str] = &["README.md", "docs/users-guide.md"];
const MARKER_PREFIX: &str = "<!-- tested-example: ";
const MARKER_SUFFIX: &str = " -->";

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
        match parse_marker(line) {
            Some(id) => {
                ensure!(ids.insert(id), "duplicate tested-example identifier '{id}'");
                examples.push(read_marked_example(source, line_index, id, &mut lines)?);
            }
            None => reject_unmarked_fence(source, line_index, line)?,
        }
    }

    Ok(examples)
}

fn read_marked_example<'a>(
    source: &'static str,
    marker_index: usize,
    id: &str,
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
) -> Result<DocumentedExample> {
    let (fence_index, fence) = next_non_empty_line(lines)
        .with_context(|| format!("{source}:{} marker has no fence", marker_index + 1))?;
    let language = fence.strip_prefix("```").with_context(|| {
        format!(
            "{source}:{} expected an opening fence after marker",
            fence_index + 1
        )
    })?;
    ensure!(
        !language.is_empty(),
        "{source}:{} fence should declare a language",
        fence_index + 1
    );
    let body = read_fence_body(source, fence_index, lines)?;
    Ok(DocumentedExample {
        id: id.to_owned(),
        language: language.to_owned(),
        body,
    })
}

fn reject_unmarked_fence(source: &str, line_index: usize, line: &str) -> Result<()> {
    ensure!(
        !line.starts_with("```"),
        "{source}:{} fence is missing a tested-example marker",
        line_index + 1
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
