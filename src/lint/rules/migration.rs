//! Rules that detect workarounds for behaviour a release has since changed.
//!
//! A stale workaround is worse than no workaround: it was correct once, so it
//! looks deliberate, and the manifest still compiles. Both rules here police
//! the Ninja escaping boundary that
//! `docs/adr-014-backend-text-escaping-seam.md` moved.

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
        Registered::Document(&ManualNinjaEscape),
        Registered::Document(&LegacyPlaceholder),
    ]
}

/// Detects the pre-v0.1.0 doubled-dollar escaping workaround.
pub struct ManualNinjaEscape;

/// Metadata for [`ManualNinjaEscape`].
static MANUAL_NINJA_ESCAPE: RuleMeta = RuleMeta {
    name: "manual-ninja-escape",
    category: Category::Migration,
    stage: Stage::Document,
    default_severity: DefaultSeverity::On(Severity::Warning),
    summary: "recipe doubles a dollar to escape it for Ninja",
    rationale: concat!(
        "Netsuke now escapes dollars at the Ninja writer boundary, after it has ",
        "lowered its own placeholders. A recipe that still doubles a dollar ",
        "reaches the shell as a literal `$$`, whose first two characters expand ",
        "to the shell's process identifier rather than to the intended variable."
    ),
    remediation: "Write the shell variable normally, for example `$PATH` rather than `$$PATH`.",
};

impl DocumentRule for ManualNinjaEscape {
    fn meta(&self) -> &'static RuleMeta {
        &MANUAL_NINJA_ESCAPE
    }

    fn check(&self, doc: &Document, sink: &mut FindingSink<'_>) {
        for part in recipes::parts(doc) {
            report_escapes(&part, sink);
        }
    }
}

/// Report every retired escaping workaround in one recipe fragment.
fn report_escapes(part: &RecipePart<'_>, sink: &mut FindingSink<'_>) {
    let offending = shellscan::find_all(part.source, "$$")
        .into_iter()
        .filter(|found| opens_variable(part.source, found.start));
    for found in offending {
        sink.at(
            part.sub_span(found.start, 2),
            format!(
                "{} `{}` escapes a dollar for Ninja, which Netsuke now does itself",
                part.item.label(),
                part.kind.key()
            ),
        );
    }
}

/// Report whether the `$$` at `start` is followed by a variable name.
///
/// A bare `$$` is the shell's own process identifier and is left alone; only a
/// doubled dollar introducing a name looks like the retired escaping
/// workaround.
fn opens_variable(text: &str, start: usize) -> bool {
    text.as_bytes()
        .get(start + 2)
        .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'{'))
}

/// Detects the undocumented `$in` and `$out` placeholder spellings.
pub struct LegacyPlaceholder;

/// Metadata for [`LegacyPlaceholder`].
static LEGACY_PLACEHOLDER: RuleMeta = RuleMeta {
    name: "legacy-placeholder",
    category: Category::Migration,
    stage: Stage::Document,
    default_severity: DefaultSeverity::On(Severity::Warning),
    summary: "recipe uses the undocumented `$in` or `$out` placeholder",
    rationale: concat!(
        "Netsuke substitutes `$in` and `$out` while lowering a recipe, but the ",
        "users' guide documents only `{{ ins }}` and `{{ outs }}`. A recipe that ",
        "meant the shell variable of the same name is silently rewritten, and a ",
        "reader cannot tell the two intentions apart."
    ),
    remediation: "Write `{{ ins }}` or `{{ outs }}` for Netsuke's paths, and rename any shell variable that collides.",
};

impl DocumentRule for LegacyPlaceholder {
    fn meta(&self) -> &'static RuleMeta {
        &LEGACY_PLACEHOLDER
    }

    fn check(&self, doc: &Document, sink: &mut FindingSink<'_>) {
        for part in recipes::parts(doc) {
            for (token, replacement) in PLACEHOLDERS {
                report_placeholders(&part, token, replacement, sink);
            }
        }
    }
}

/// The legacy placeholders and the documented spellings that replace them.
static PLACEHOLDERS: [(&str, &str); 2] = [("$in", "{{ ins }}"), ("$out", "{{ outs }}")];

/// Report every bare use of one legacy placeholder in a recipe fragment.
fn report_placeholders(
    part: &RecipePart<'_>,
    token: &str,
    replacement: &str,
    sink: &mut FindingSink<'_>,
) {
    let offending = shellscan::find_all(part.source, token)
        .into_iter()
        .filter(|found| is_bare_placeholder(part.source, found.start, token.len()));
    for found in offending {
        sink.at(
            part.sub_span(found.start, token.len()),
            format!(
                "{} `{}` uses `{token}`; write `{replacement}` instead",
                part.item.label(),
                part.kind.key()
            ),
        );
    }
}

/// Report whether the placeholder at `start` stands alone.
///
/// A preceding dollar makes it an escaping workaround, which
/// [`ManualNinjaEscape`] owns, and a trailing name byte makes it a longer
/// variable such as `$output`.
fn is_bare_placeholder(text: &str, start: usize, len: usize) -> bool {
    let bytes = text.as_bytes();
    let preceded_by_dollar = start
        .checked_sub(1)
        .and_then(|index| bytes.get(index))
        .is_some_and(|byte| *byte == b'$');
    let continues = bytes
        .get(start + len)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
    !preceded_by_dollar && !continues
}

#[cfg(test)]
#[path = "migration_tests.rs"]
mod tests;
