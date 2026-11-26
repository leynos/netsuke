//! Classification of one generated sample into its material classes.
//!
//! Each classifier names the classes its owning property depends on. The
//! lists those properties require live in `classes.rs`; this module only
//! counts what a draw contains.

use super::super::strategy::{
    BodySpec, ControlOrder, DocumentSpec, EntrySpec, ForeachForm, Items, Section,
};
use super::{ClassSet, count};
use crate::manifest::ManifestValue;

/// Count the JSON kind of one value, and whether it nests another container.
fn value_classes(value: &ManifestValue, depth: usize, tally: &mut ClassSet) {
    match value {
        ManifestValue::Null => count(tally, "value-null"),
        ManifestValue::Bool(_) => count(tally, "value-bool"),
        ManifestValue::Number(_) => count(tally, "value-number"),
        ManifestValue::String(text) => {
            count(tally, "value-string");
            if text.contains("{{") {
                count(tally, "value-jinja-string");
            }
        }
        ManifestValue::Array(items) => {
            count(tally, "value-array");
            if depth >= 1 {
                count(tally, "value-nested");
            }
            for item in items {
                value_classes(item, depth + 1, tally);
            }
        }
        ManifestValue::Object(map) => {
            count(tally, "value-object");
            if depth >= 1 {
                count(tally, "value-nested");
            }
            for nested in map.values() {
                value_classes(nested, depth + 1, tally);
            }
        }
    }
}

/// Classify one general entry's section, control-key layout, and bodies.
pub(super) fn entry_classes(
    spec: &super::super::strategy::EntrySpec,
    tally: &mut ClassSet,
) -> bool {
    let body = &spec.body;
    count(tally, spec.section.name());
    count(
        tally,
        if body.is_foreach() {
            "foreach"
        } else {
            "static"
        },
    );
    count(tally, if body.has_vars() { "vars" } else { "no-vars" });
    condition_classes(body, tally);
    count(tally, control_class(body));
    count(tally, control_order_class(body));
    count(tally, control_adjacency_class(body));
    foreach_classes(body, tally);
    value_and_vars_classes(body, tally);
    true
}

/// Classify a condition's presence, form, and free names.
fn condition_classes(body: &super::super::strategy::BodySpec, tally: &mut ClassSet) {
    let Some(condition) = body.condition() else {
        count(tally, "no-condition");
        return;
    };
    count(tally, "condition");
    count(tally, condition_form_class(body));
    if condition.variable().is_some() {
        count(tally, "condition-reads-vars");
    }
    if condition.reads_index() {
        count(tally, "condition-reads-index");
    }
    if condition.reads_item() {
        count(tally, "condition-reads-item");
    }
}

/// Name the condition's spelling class.
fn condition_form_class(body: &super::super::strategy::BodySpec) -> &'static str {
    if body.condition_is_template() {
        "condition-template"
    } else {
        "condition-expression"
    }
}

/// Name which control key the entry writes first, if it writes both.
fn control_order_class(body: &BodySpec) -> &'static str {
    match (body.is_foreach(), body.has_condition()) {
        (true, true) => match body.control_order() {
            ControlOrder::ForeachFirst => "control-both-foreach-first",
            ControlOrder::WhenFirst => "control-both-when-first",
        },
        _ => "control-single",
    }
}

/// Name whether the two control keys share one slot or take separate ones.
fn control_adjacency_class(body: &BodySpec) -> &'static str {
    if !(body.is_foreach() && body.has_condition()) {
        return "control-slot-inapplicable";
    }
    if body.control_same_slot() {
        "control-same-slot"
    } else {
        "control-two-slots"
    }
}

/// Name the class of keys that follow the control block.
fn control_class(body: &super::super::strategy::BodySpec) -> &'static str {
    match body.following_key_count() {
        0 => "control-last",
        1 => "control-one-following",
        _ => "control-two-or-more-following",
    }
}

/// Classify the foreach spelling and whether the value list is empty.
fn foreach_classes(body: &super::super::strategy::BodySpec, tally: &mut ClassSet) {
    let Some(items) = body.items() else {
        return;
    };
    count(
        tally,
        if items.len() == 0 {
            "foreach-empty"
        } else {
            "foreach-non-empty"
        },
    );
    count(
        tally,
        match body.foreach_form() {
            Some(super::super::strategy::ForeachForm::InlineArray) => "foreach-array",
            Some(super::super::strategy::ForeachForm::ListLiteral) => "foreach-string",
            None => "foreach-missing-form",
        },
    );
}

/// Classify every authored field and `vars` value by JSON kind.
fn value_and_vars_classes(body: &super::super::strategy::BodySpec, tally: &mut ClassSet) {
    for (_, value) in body.fields() {
        value_classes(value, 0, tally);
    }
    for key in body.vars_keys() {
        if let Some(value) = body.vars_value(key) {
            value_classes(value, 0, tally);
        }
    }
}

/// Count the classes of a sample whose control block is forced to the end.
///
/// This is the negative control's classifier: it can never reach
/// `control-two-or-more-following`, so the floor check must report that class.
pub(super) fn forced_control_classes(spec: &EntrySpec, tally: &mut ClassSet) -> bool {
    let forced = spec.body.clone().with_control_last();
    let class = match forced.following_key_count() {
        0 => "control-last",
        1 => "control-one-following",
        _ => "control-two-or-more-following",
    };
    count(tally, class);
    true
}

/// Classify a static-condition entry by section, verdict, and spelling.
pub(super) fn static_classes(spec: &EntrySpec, tally: &mut ClassSet) -> bool {
    let body = &spec.body;
    if !body.has_condition() {
        return false;
    }
    let verdict_holds = super::super::model::expected_filtered(body).is_empty();
    let class = match (spec.section, verdict_holds, body.condition_is_template()) {
        (Section::Targets, true, false) => "targets-held-expression",
        (Section::Targets, true, true) => "targets-held-template",
        (Section::Targets, false, false) => "targets-failed-expression",
        (Section::Targets, false, true) => "targets-failed-template",
        (Section::Actions, true, false) => "actions-held-expression",
        (Section::Actions, true, true) => "actions-held-template",
        (Section::Actions, false, false) => "actions-failed-expression",
        (Section::Actions, false, true) => "actions-failed-template",
    };
    count(tally, class);
    if body
        .condition()
        .is_some_and(|condition| condition.variable().is_some())
    {
        count(tally, "reads-vars");
    }
    true
}

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
            count(tally, section_verdict_class(section, &entry.body));
        }
    }
    true
}

/// Name one entry's condition verdict within its section.
///
/// `OBL-WHEN-REMOVED` requires, per section, entries whose condition held,
/// entries whose condition failed, and entries with no condition; the document
/// classifier must reach all three or the property could pass without ever
/// exercising one of them.
fn section_verdict_class(section: Section, body: &BodySpec) -> &'static str {
    if !body.has_condition() {
        return match section {
            Section::Targets => "targets-entry-absent",
            Section::Actions => "actions-entry-absent",
        };
    }
    let held = super::super::model::expected_filtered(body).is_empty();
    match (section, held) {
        (Section::Targets, true) => "targets-entry-held",
        (Section::Targets, false) => "targets-entry-failed",
        (Section::Actions, true) => "actions-entry-held",
        (Section::Actions, false) => "actions-entry-failed",
    }
}

/// Classify one foreach entry for the injection obligations.
pub(super) fn injection_classes(spec: &EntrySpec, tally: &mut ClassSet) -> bool {
    let body = &spec.body;
    if let Some(form) = body.foreach_form() {
        count(tally, foreach_form_class(form));
    }
    user_collisions(body, tally);
    let Some(items) = body.items() else {
        return false;
    };
    let included: Vec<usize> = (0..items.len())
        .filter(|index| body.condition_holds(items.value_at(*index).as_i64(), *index))
        .collect();
    count(tally, inclusion_class(items.len(), &included));
    if has_gap(items.len(), &included) {
        count(tally, "inject-gap");
    }
    item_classes(body, items, tally);
    true
}

/// Count the item kinds one foreach entry's values reach.
fn item_classes(body: &BodySpec, items: &Items, tally: &mut ClassSet) {
    let Items::Arbitrary(values) = items else {
        // An empty integer sequence reaches no item kind at all; reporting
        // one here would credit a class the draw never contained.
        if items.len() > 0 {
            count(tally, "item-number");
        }
        return;
    };
    let string_form = body.foreach_form() == Some(ForeachForm::ListLiteral);
    for value in values {
        let kind = match value {
            ManifestValue::Null => {
                if string_form {
                    count(tally, "item-null-string");
                }
                "item-null"
            }
            ManifestValue::Bool(_) => "item-bool",
            ManifestValue::Number(_) => "item-number",
            ManifestValue::String(_) => "item-string",
            ManifestValue::Array(_) => "item-array",
            ManifestValue::Object(_) => "item-object",
        };
        count(tally, kind);
    }
}

/// Name the class of one `foreach` spelling.
fn foreach_form_class(form: ForeachForm) -> &'static str {
    match form {
        ForeachForm::InlineArray => "foreach-array",
        ForeachForm::ListLiteral => "foreach-string",
    }
}

/// Count a user `vars` key that collides with an injected name.
fn user_collisions(body: &BodySpec, tally: &mut ClassSet) {
    let keys = body.vars_keys();
    if keys.contains(&"item") {
        count(tally, "user-item-collision");
    }
    if keys.contains(&"index") {
        count(tally, "user-index-collision");
    }
}

/// Name the selection class of one iteration sequence.
fn inclusion_class(item_count: usize, included: &[usize]) -> &'static str {
    if item_count == 0 {
        "inject-empty"
    } else if included.len() == item_count {
        "inject-all-included"
    } else if included.is_empty() {
        "inject-all-excluded"
    } else {
        "inject-partial"
    }
}

/// Report whether an excluded item precedes an included one.
///
/// This is the only selection shape on which renumbering an injected index is
/// visible: a prefix-shaped selection keeps true and renumbered indexes equal.
fn has_gap(item_count: usize, included: &[usize]) -> bool {
    (0..item_count)
        .any(|index| !included.contains(&index) && included.iter().any(|kept| *kept > index))
}
