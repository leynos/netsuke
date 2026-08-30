//! Rules about the discovery metadata a manifest exposes.

use crate::lint::document::Document;
use crate::lint::registry::Registered;
use crate::lint::rule::{Category, DocumentRule, FindingSink, RuleMeta, Stage};
use crate::lint::severity::{DefaultSeverity, Severity};

use crate::lint::rules::recipes::{self, Section};

/// Register this module's rules.
#[must_use]
pub fn rules() -> Vec<Registered> {
    vec![
        Registered::Document(&ActionWithoutDescription),
        Registered::Document(&RuleWithoutDescription),
    ]
}

/// Detects actions that carry no discovery metadata.
pub struct ActionWithoutDescription;

/// Metadata for [`ActionWithoutDescription`].
static ACTION_WITHOUT_DESCRIPTION: RuleMeta = RuleMeta {
    name: "action-without-description",
    category: Category::Clarity,
    stage: Stage::Document,
    default_severity: DefaultSeverity::On(Severity::Advice),
    summary: "action declares no `description`",
    rationale: concat!(
        "Actions are a manifest's public entry points, and `netsuke help ",
        "targets` is how a newcomer or an agent discovers them. An action ",
        "without a `description` appears in that catalogue with no explanation, ",
        "so the only way to learn what it does is to read its recipe."
    ),
    remediation: "Add a `description` stating the operation the action performs.",
};

impl DocumentRule for ActionWithoutDescription {
    fn meta(&self) -> &'static RuleMeta {
        &ACTION_WITHOUT_DESCRIPTION
    }

    fn check(&self, doc: &Document, sink: &mut FindingSink<'_>) {
        report_missing_descriptions(
            doc,
            Section::Actions,
            "is not described in `netsuke help targets`",
            sink,
        );
    }
}

/// Detects rules that carry no Ninja progress text.
pub struct RuleWithoutDescription;

/// Metadata for [`RuleWithoutDescription`].
static RULE_WITHOUT_DESCRIPTION: RuleMeta = RuleMeta {
    name: "rule-without-description",
    category: Category::Clarity,
    stage: Stage::Document,
    default_severity: DefaultSeverity::Off,
    summary: "rule declares no `description`",
    rationale: concat!(
        "Ninja shows a rule's `description` as it runs. Without one it prints ",
        "the whole command line, which is noisy for a long compiler invocation ",
        "and makes a build log hard to scan."
    ),
    remediation: "Add a `description` naming the work the rule performs, for example `Compiling an object file`.",
};

impl DocumentRule for RuleWithoutDescription {
    /// This rule is off by default: descriptions are a house style rather than
    /// a correctness property, so it runs only when a project selects it.
    fn meta(&self) -> &'static RuleMeta {
        &RULE_WITHOUT_DESCRIPTION
    }

    fn check(&self, doc: &Document, sink: &mut FindingSink<'_>) {
        report_missing_descriptions(
            doc,
            Section::Rules,
            "shows its whole command line as Ninja progress text",
            sink,
        );
    }
}

/// Report every item of `section` that declares no `description`.
fn report_missing_descriptions(
    doc: &Document,
    section: Section,
    consequence: &str,
    sink: &mut FindingSink<'_>,
) {
    for item in recipes::items(doc)
        .into_iter()
        .filter(|item| item.section == section && item.node.get("description").is_none())
    {
        sink.at(
            item.field_span("name"),
            format!("{} {consequence}", item.label()),
        );
    }
}

#[cfg(test)]
#[path = "descriptions_tests.rs"]
mod tests;
