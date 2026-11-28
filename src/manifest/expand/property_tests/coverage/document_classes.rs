//! Classification of whole documents for the section-level obligations.

use super::super::strategy::{BodySpec, DocumentSpec, Section};
use super::{ClassSet, count};

/// Classify a whole document's sections and entry mix.
pub(super) fn document_classes(spec: &DocumentSpec, tally: &mut ClassSet) -> bool {
    if !spec.targets.is_empty() {
        count(tally, "targets-entries");
    }
    if !spec.actions.is_empty() {
        count(tally, "actions-entries");
    }
    for section in [Section::Targets, Section::Actions] {
        for entry in spec.entries(section) {
            count(
                tally,
                if entry.body.is_foreach() {
                    "document-foreach"
                } else {
                    "document-static"
                },
            );
            section_verdict_classes(section, &entry.body, tally);
            if entry
                .body
                .condition()
                .is_some_and(|condition| condition.variable().is_some())
            {
                count(tally, "reads-vars");
            }
        }
    }
    true
}

/// Name one entry's condition verdicts within its section.
fn section_verdict_classes(section: Section, body: &BodySpec, tally: &mut ClassSet) {
    if !body.has_condition() {
        count(tally, section_class(section, "absent"));
        return;
    }
    if let Some(items) = body.items() {
        let verdicts: Vec<bool> = (0..items.len())
            .map(|index| body.condition_holds(items.value_at(index).as_i64(), index))
            .collect();
        if verdicts.is_empty() {
            // No iteration ran, so no condition was evaluated.
            count(tally, section_class(section, "unevaluated"));
        }
        if verdicts.iter().any(|held| *held) {
            count(tally, section_class(section, "held"));
            count(tally, section_form_class(section, body, "held"));
        }
        if verdicts.iter().any(|held| !*held) {
            count(tally, section_class(section, "failed"));
            count(tally, section_form_class(section, body, "failed"));
        }
        return;
    }
    let held = body.condition_holds(None, 0);
    let verdict = if held { "held" } else { "failed" };
    count(tally, section_class(section, verdict));
    count(tally, section_form_class(section, body, verdict));
}

/// Name one section, verdict, and condition-spelling class.
fn section_form_class(section: Section, body: &BodySpec, verdict: &'static str) -> &'static str {
    let form = if body.condition_is_template() {
        "template"
    } else {
        "expression"
    };
    match (section, verdict, form) {
        (Section::Targets, "held", "expression") => "targets-held-expression",
        (Section::Targets, "held", _) => "targets-held-template",
        (Section::Targets, _, "expression") => "targets-failed-expression",
        (Section::Targets, _, _) => "targets-failed-template",
        (Section::Actions, "held", "expression") => "actions-held-expression",
        (Section::Actions, "held", _) => "actions-held-template",
        (Section::Actions, _, "expression") => "actions-failed-expression",
        (Section::Actions, _, _) => "actions-failed-template",
    }
}

/// Name one section-and-verdict class.
fn section_class(section: Section, verdict: &'static str) -> &'static str {
    match (section, verdict) {
        (Section::Targets, "absent") => "targets-entry-absent",
        (Section::Targets, "held") => "targets-entry-held",
        (Section::Targets, "failed") => "targets-entry-failed",
        (Section::Targets, _) => "targets-entry-unevaluated",
        (Section::Actions, "absent") => "actions-entry-absent",
        (Section::Actions, "held") => "actions-entry-held",
        (Section::Actions, "failed") => "actions-entry-failed",
        (Section::Actions, _) => "actions-entry-unevaluated",
    }
}
