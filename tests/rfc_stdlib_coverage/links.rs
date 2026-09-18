//! Resolution of relative Markdown links across the RFC corpus.
//!
//! Nothing else in the toolchain checks these. `markdownlint-cli2` is configured
//! without cross-file link or anchor validation, and `mdtablefix` only
//! canonicalises tables, so a relative link between two documents can rot
//! unnoticed. The RFC corpus is where that matters most: the child RFCs cite
//! each other, RFC 0006, the roadmap, and the ADRs, and a reader following a
//! stale path has no fallback.

use anyhow::{Context, Result};

use super::Repo;

/// A relative link target found in a document.
pub(super) struct Target {
    /// The target as written, before resolution.
    pub(super) target: String,
    /// One-indexed line the target was found on.
    pub(super) line: usize,
}

/// Every relative Markdown link target in `text`.
///
/// Absolute URLs, bare anchors, and `mailto:` targets are skipped: none is a
/// path this test can resolve. Targets that wrap across lines are read whole,
/// because a link split by the 80-column wrap is still a link.
pub(super) fn targets(text: &str) -> Vec<Target> {
    let mut found = Vec::new();
    let mut line = 1;
    let mut rest = text;
    while let Some(open) = rest.find("](") {
        // `open` indexes the `]` of a `](`, and every byte skipped here is ASCII,
        // so each split lands on a character boundary.
        let (head, tail) = rest.split_at(open);
        line += head.matches('\n').count();
        let after = tail.split_at(2).1;
        let Some(close) = after.find(')') else {
            break;
        };
        let (raw, remainder) = after.split_at(close);
        let target = raw.split_whitespace().next().unwrap_or("").to_owned();
        if is_relative(&target) {
            found.push(Target { target, line });
        }
        line += raw.matches('\n').count();
        rest = remainder.split_at(1).1;
    }
    found
}

/// Whether a link target is a relative path this test should resolve.
pub(super) fn is_relative(target: &str) -> bool {
    !target.is_empty()
        && !target.starts_with('#')
        && !target.contains("://")
        && !target.starts_with("mailto:")
}

/// The path part of a link target, with any `#fragment` removed.
///
/// A target that is nothing but a fragment is returned unchanged, so a bare
/// anchor that reached here still fails to resolve rather than passing as the
/// directory it sits in.
fn path_of(target: &str) -> &str {
    let (path, _) = target.split_once('#').unwrap_or((target, ""));
    if path.is_empty() { target } else { path }
}

/// Resolve a relative target against the directory holding `file`.
///
/// Only `..` segments are collapsed, which is all the RFC corpus uses. A target
/// that escapes the repository root is returned unchanged and will simply fail
/// to resolve, which is the right outcome for a link that points outside.
///
/// A `#fragment` selects a heading inside the target document, so only the path
/// before it names a file. Fragments are stripped and not validated: the anchor
/// slugs are a renderer's business, and every renderer spells them differently.
pub(super) fn resolve(file: &str, target: &str) -> String {
    let path = path_of(target);
    let mut segments: Vec<&str> = file.split('/').collect();
    segments.pop();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    segments.join("/")
}

/// Every dangling relative link in `file`.
///
/// Returns the failures rather than asserting, so a caller can report all of
/// them at once instead of one per run.
pub(super) fn dangling_in(repo: &Repo, file: &str) -> Result<Vec<String>> {
    let text = repo.read(file)?;
    let mut failures = Vec::new();
    for target in targets(&text) {
        let path = resolve(file, &target.target);
        if !repo
            .exists(&path)
            .with_context(|| format!("resolve {}:{} -> {path}", file, target.line))?
        {
            failures.push(format!(
                "{file}:{} links to {} which resolves to {path}, and no such file exists",
                target.line, target.target
            ));
        }
    }
    Ok(failures)
}

/// Every dangling relative link in the RFC corpus.
pub(super) fn dangling(repo: &Repo) -> Result<Vec<String>> {
    let mut failures = Vec::new();
    for file in repo.markdown_files(super::RFC_DIR)? {
        failures.extend(dangling_in(repo, &file)?);
    }
    Ok(failures)
}
