//! Rules about recipes whose result does not follow from their inputs.
//!
//! Netsuke's premise is one static graph decided before Ninja starts. A recipe
//! that detaches a process or re-enters a build tool moves work outside that
//! graph, where Netsuke can neither order it nor know when it finished.

use crate::lint::document::Document;
use crate::lint::registry::Registered;
use crate::lint::rule::{Category, DocumentRule, FindingSink, RuleMeta, Stage};
use crate::lint::severity::{DefaultSeverity, Severity};

use super::recipes::{self, RecipePart};
use super::shellscan;

/// Register this module's rules.
#[must_use]
pub fn rules() -> Vec<Registered> {
    vec![
        Registered::Document(&BackgroundJob),
        Registered::Document(&RecursiveBuildInvocation),
    ]
}

/// Detects recipes that detach a process.
pub struct BackgroundJob;

/// Metadata for [`BackgroundJob`].
static BACKGROUND_JOB: RuleMeta = RuleMeta {
    name: "background-job",
    category: Category::Determinism,
    stage: Stage::Document,
    default_severity: DefaultSeverity::On(Severity::Warning),
    summary: "recipe detaches a process with a trailing `&`",
    rationale: concat!(
        "A detached process outlives the recipe that started it. Netsuke marks ",
        "the target complete as soon as the shell returns, so a later target ",
        "can consume a half-written output, and the build can finish while work ",
        "is still running."
    ),
    remediation: "Run the command in the foreground, or move the detached work outside the build into a separate command.",
};

impl DocumentRule for BackgroundJob {
    fn meta(&self) -> &'static RuleMeta {
        &BACKGROUND_JOB
    }

    fn check(&self, doc: &Document, sink: &mut FindingSink<'_>) {
        for part in recipes::parts(doc) {
            for offset in detached_offsets(&part) {
                sink.at(
                    part.sub_span(offset, 1),
                    format!(
                        "{} `{}` detaches a process",
                        part.item.label(),
                        part.kind.key()
                    ),
                );
            }
        }
    }
}

/// Report the offsets of every shell-active `&` that detaches a command.
///
/// A `&` is only backgrounding when it terminates a command, so the scan looks
/// for one that is not part of `&&`, not a redirection such as `2>&1`, and is
/// followed only by whitespace or the end of a line.
fn detached_offsets(part: &RecipePart<'_>) -> Vec<usize> {
    let bytes = part.source.as_bytes();
    let mask = shellscan::Mask::new(part.source);
    bytes
        .iter()
        .enumerate()
        .filter(|(index, byte)| **byte == b'&' && mask.is_active(*index))
        .filter(|(index, _)| detaches(bytes, *index))
        .map(|(index, _)| index)
        .collect()
}

/// Report whether the `&` at `index` terminates a command.
///
/// A `&` that is part of `&&`, or of a redirection such as `2>&1`, joins
/// commands rather than detaching one.
fn detaches(bytes: &[u8], index: usize) -> bool {
    let previous = index.checked_sub(1).and_then(|prior| bytes.get(prior));
    let next = bytes.get(index.saturating_add(1));
    !matches!(previous, Some(b'&' | b'>' | b'<'))
        && !matches!(next, Some(b'&' | b'>' | b'1' | b'2'))
        && next.is_none_or(u8::is_ascii_whitespace)
}

/// Build tools whose re-entry defeats the single static graph.
static BUILD_TOOLS: [&str; 3] = ["netsuke", "make", "ninja"];

/// Detects recipes that invoke a build tool.
pub struct RecursiveBuildInvocation;

/// Metadata for [`RecursiveBuildInvocation`].
static RECURSIVE_BUILD_INVOCATION: RuleMeta = RuleMeta {
    name: "recursive-build-invocation",
    category: Category::Determinism,
    stage: Stage::Document,
    default_severity: DefaultSeverity::On(Severity::Warning),
    summary: "recipe invokes a build tool",
    rationale: concat!(
        "Netsuke decides the whole graph before Ninja starts. A recipe that ",
        "invokes `netsuke`, `make`, or `ninja` hides a second graph inside one ",
        "edge, so Netsuke cannot order the two, cannot schedule them against one ",
        "job budget, and cannot tell whether the inner build's inputs changed."
    ),
    remediation: "Declare the inner build's work as targets in this manifest so one graph owns all of it.",
};

impl DocumentRule for RecursiveBuildInvocation {
    fn meta(&self) -> &'static RuleMeta {
        &RECURSIVE_BUILD_INVOCATION
    }

    fn check(&self, doc: &Document, sink: &mut FindingSink<'_>) {
        for part in recipes::parts(doc) {
            report_invocations(&part, sink);
        }
    }
}

/// Report every build-tool invocation in one recipe fragment.
fn report_invocations(part: &RecipePart<'_>, sink: &mut FindingSink<'_>) {
    let invocations = shellscan::segments(part.source)
        .into_iter()
        .filter_map(|(offset, segment)| Some((offset, shellscan::leading_word(segment)?)))
        .filter_map(|(offset, (lead, word))| Some((offset, lead, word, invoked_tool(word)?)));
    for (offset, lead, word, tool) in invocations {
        sink.at(
            part.sub_span(offset.saturating_add(lead), word.len()),
            format!(
                "{} `{}` invokes `{tool}`",
                part.item.label(),
                part.kind.key()
            ),
        );
    }
}

/// Report the build tool a leading word invokes, ignoring any path prefix.
fn invoked_tool(word: &str) -> Option<&'static str> {
    let command = word.rsplit('/').next().unwrap_or(word);
    BUILD_TOOLS
        .into_iter()
        .find(|tool| command == *tool || command == format!("{tool}.exe"))
}

#[cfg(test)]
#[path = "determinism_tests.rs"]
mod tests;
