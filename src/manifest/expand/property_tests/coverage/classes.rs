//! Class lists the coverage tripwires require.
//!
//! Each list names a class its owning property depends on. A class that
//! disappears from the generator silently weakens the property, so the lists
//! live beside the classifier that counts them.

/// Classes the general entry generator must reach.
pub(super) const ENTRY_CLASSES: &[&str] = &[
    "targets",
    "actions",
    "static",
    "foreach",
    "vars",
    "no-vars",
    "condition",
    "no-condition",
    "condition-expression",
    "condition-template",
    "condition-reads-vars",
    "condition-reads-index",
    "condition-reads-item",
    "control-last",
    "control-one-following",
    "control-two-or-more-following",
    "foreach-empty",
    "foreach-non-empty",
    "foreach-array",
    "foreach-string",
    "value-null",
    "value-bool",
    "value-number",
    "value-string",
    "value-array",
    "value-object",
    "value-nested",
    "value-jinja-string",
];

/// Classes the foreach generator must reach for the injection obligations.
pub(super) const INJECT_CLASSES: &[&str] = &[
    "foreach-array",
    "foreach-string",
    "inject-empty",
    "inject-all-included",
    "inject-all-excluded",
    "inject-gap",
    "item-null",
    "item-bool",
    "item-number",
    "item-string",
    "item-array",
    "item-object",
    "item-null-string",
    "user-item-collision",
    "user-index-collision",
];

/// Classes the static-condition generator must reach.
pub(super) const STATIC_CLASSES: &[&str] = &[
    "targets-held-expression",
    "targets-held-template",
    "targets-failed-expression",
    "targets-failed-template",
    "actions-held-expression",
    "actions-held-template",
    "actions-failed-expression",
    "actions-failed-template",
    "reads-vars",
];

/// Classes the document generator must reach.
pub(super) const DOCUMENT_CLASSES: &[&str] = &[
    "targets-entries",
    "actions-entries",
    "document-static",
    "document-foreach",
    "document-condition",
];
