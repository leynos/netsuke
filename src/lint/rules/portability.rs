//! Rules about constructs that depend on a shell Netsuke does not promise.
//!
//! Netsuke runs `script` recipes under `/bin/sh -e` and does not abstract over
//! PowerShell. A construct that only `bash` implements therefore fails on a
//! host whose `/bin/sh` is `dash`, which is the default on several
//! distributions and inside many container images.

use crate::lint::document::Document;
use crate::lint::registry::Registered;
use crate::lint::rule::{Category, DocumentRule, FindingSink, RuleMeta, Stage};
use crate::lint::severity::{DefaultSeverity, Severity};

use super::recipes::{self, RecipePart};
use super::shellscan;

/// Register this module's rules.
#[must_use]
pub fn rules() -> Vec<Registered> {
    vec![Registered::Document(&Bashism)]
}

/// Where a construct must appear before it counts as shell syntax.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Position {
    /// Anywhere in the shell-active text.
    Anywhere,
    /// Bounded by non-word bytes, so it does not match inside a longer word.
    Word,
    /// Only as the leading word of a command.
    ///
    /// `function`, `source`, and `local` are ordinary arguments elsewhere:
    /// `grep function main.c` names a search pattern, not a shell keyword.
    Command,
}

/// One non-portable construct and the portable alternative.
struct Construct {
    /// The literal text that identifies the construct.
    token: &'static str,
    /// Where the token must appear to count.
    position: Position,
    /// What to write instead.
    advice: &'static str,
}

/// Constructs `bash` implements that `/bin/sh` does not promise.
///
/// The list is deliberately short and literal. Every entry is a construct that
/// `dash` rejects outright rather than one it merely implements differently,
/// so a match is a portability defect rather than a style preference.
static CONSTRUCTS: &[Construct] = &[
    Construct {
        token: "[[",
        position: Position::Anywhere,
        advice: "use `[` with POSIX operators",
    },
    Construct {
        token: "function",
        position: Position::Command,
        advice: "declare the function as `name() { … }`",
    },
    Construct {
        token: "source",
        position: Position::Command,
        advice: "use `.` to source a file",
    },
    Construct {
        token: "local",
        position: Position::Command,
        advice: "assign without `local`, or keep the assignment in a subshell",
    },
    Construct {
        token: "<<<",
        position: Position::Anywhere,
        advice: "pipe the value in with `printf %s … |`",
    },
    Construct {
        token: "&>",
        position: Position::Anywhere,
        advice: "redirect with `> file 2>&1`",
    },
    Construct {
        token: "|&",
        position: Position::Anywhere,
        advice: "redirect with `2>&1 |`",
    },
    Construct {
        token: "pipefail",
        position: Position::Word,
        advice: "check each stage's status, or accept the pipeline's last status",
    },
    Construct {
        token: "echo -e",
        position: Position::Anywhere,
        advice: "use `printf` for escape sequences",
    },
];

/// Detects `bash`-only constructs in recipes run by `/bin/sh`.
pub struct Bashism;

/// Metadata for [`Bashism`].
static BASHISM: RuleMeta = RuleMeta {
    name: "bashism",
    category: Category::Portability,
    stage: Stage::Document,
    default_severity: DefaultSeverity::On(Severity::Warning),
    summary: "recipe uses a construct `/bin/sh` does not promise",
    rationale: concat!(
        "Netsuke runs `script` recipes under `/bin/sh -e`, and `command` ",
        "recipes through the same shell. On a host where `/bin/sh` is `dash` ",
        "rather than `bash`, a `bash`-only construct fails at build time with a ",
        "syntax error that does not reproduce on the author's machine."
    ),
    remediation: "Rewrite the construct in POSIX shell, or move the work into a script the manifest invokes.",
};

impl DocumentRule for Bashism {
    fn meta(&self) -> &'static RuleMeta {
        &BASHISM
    }

    fn check(&self, doc: &Document, sink: &mut FindingSink<'_>) {
        for part in recipes::parts(doc) {
            for construct in CONSTRUCTS {
                report_construct(&part, construct, sink);
            }
        }
    }
}

/// Report every occurrence of one non-portable construct in a fragment.
fn report_construct(part: &RecipePart<'_>, construct: &Construct, sink: &mut FindingSink<'_>) {
    let matches = match construct.position {
        Position::Anywhere => shellscan::find_all(part.source, construct.token),
        Position::Word => shellscan::find_words(part.source, construct.token),
        Position::Command => command_positions(part.source, construct.token),
    };
    for found in matches {
        sink.at(
            part.sub_span(found.start, found.len),
            format!(
                "{} `{}` uses `{}`, which `/bin/sh` does not promise; {}",
                part.item.label(),
                part.kind.key(),
                construct.token,
                construct.advice
            ),
        );
    }
}

/// Find every occurrence of `token` that leads a command.
fn command_positions(text: &str, token: &str) -> Vec<shellscan::Match> {
    shellscan::segments(text)
        .into_iter()
        .filter_map(|(offset, segment)| Some((offset, shellscan::leading_word(segment)?)))
        .filter(|(_, (_, word))| *word == token)
        .map(|(offset, (lead, word))| {
            shellscan::Match::new(offset.saturating_add(lead), word.len())
        })
        .collect()
}

#[cfg(test)]
#[path = "portability_tests.rs"]
mod tests;
