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
    "control-single",
    "control-both-foreach-first",
    "control-both-when-first",
    "control-same-slot",
    "control-two-slots",
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
    "control-last",
    "control-one-following",
    "control-two-or-more-following",
    "control-same-slot",
    "control-two-slots",
    "inject-empty",
    "inject-all-included",
    "inject-all-excluded",
    "inject-partial",
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
    "control-last",
    "control-one-following",
    "control-two-or-more-following",
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
    "targets-entry-held",
    "targets-entry-failed",
    "targets-entry-absent",
    "targets-entry-unevaluated",
    "actions-entry-held",
    "actions-entry-failed",
    "actions-entry-absent",
    "actions-entry-unevaluated",
];

/// Classes the static-entry generator must reach.
///
/// This is the strategy the static-survival property draws from. It carries no
/// `foreach`, so the entry-level list's iteration classes cannot occur; the
/// classes here are the ones that property depends on.
pub(super) const STATIC_ENTRY_CLASSES: &[&str] = &[
    "targets",
    "actions",
    "static",
    "vars",
    "no-vars",
    "condition",
    "no-condition",
    "condition-expression",
    "condition-template",
    "condition-reads-vars",
    "control-last",
    "control-one-following",
    "control-two-or-more-following",
];
