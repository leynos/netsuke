//! Rules that police the lint directives themselves.
//!
//! Suppression is only trustworthy while it stays accurate. A directive naming
//! a rule that no longer exists silences nothing, a directive without a reason
//! documents nothing, and a directive left behind after its problem was fixed
//! hides the next occurrence. These three rules keep all three states visible.

use crate::lint::registry::{self, Registered};
use crate::lint::rule::{Category, DirectiveContext, DirectiveRule, FindingSink, RuleMeta, Stage};
use crate::lint::severity::{DefaultSeverity, Severity};

/// Register this module's rules.
#[must_use]
pub fn rules() -> Vec<Registered> {
    vec![
        Registered::Directive(&UnknownSuppression),
        Registered::Directive(&SuppressionWithoutReason),
        Registered::Directive(&UnusedSuppression),
    ]
}

/// Detects directives naming a rule that is not registered.
pub struct UnknownSuppression;

/// Metadata for [`UnknownSuppression`].
static UNKNOWN_SUPPRESSION: RuleMeta = RuleMeta {
    name: "unknown-suppression",
    category: Category::Suppression,
    stage: Stage::Directive,
    default_severity: DefaultSeverity::On(Severity::Warning),
    summary: "lint directive names a rule that does not exist",
    rationale: concat!(
        "A directive naming an unregistered rule silences nothing. It is most ",
        "often a typo, or a rule that a Netsuke upgrade renamed or retired, and ",
        "in both cases the finding the author meant to suppress is still being ",
        "reported or is about to reappear."
    ),
    remediation: "Correct the rule name, or delete the directive. `netsuke check --explain` lists every rule.",
};

impl DirectiveRule for UnknownSuppression {
    fn meta(&self) -> &'static RuleMeta {
        &UNKNOWN_SUPPRESSION
    }

    fn check(&self, ctx: &DirectiveContext<'_>, sink: &mut FindingSink<'_>) {
        for directive in ctx.directives {
            if directive.rules.is_empty() {
                sink.at(directive.span, "lint directive names no rule".to_owned());
                continue;
            }
            for name in directive
                .rules
                .iter()
                .filter(|name| !registry::is_known(name))
            {
                sink.at(
                    directive.span,
                    format!("lint directive names unknown rule `{name}`"),
                );
            }
        }
    }
}

/// Detects directives that state no reason.
pub struct SuppressionWithoutReason;

/// Metadata for [`SuppressionWithoutReason`].
static SUPPRESSION_WITHOUT_REASON: RuleMeta = RuleMeta {
    name: "suppression-without-reason",
    category: Category::Suppression,
    stage: Stage::Directive,
    default_severity: DefaultSeverity::On(Severity::Warning),
    summary: "lint directive states no reason",
    rationale: concat!(
        "A suppression without a reason cannot be reviewed. A later reader ",
        "cannot tell a considered exception from a silenced defect, so the ",
        "directive tends to outlive whatever justified it."
    ),
    remediation: "Append `-- <reason>` to the directive, stating why the construct is correct here.",
};

impl DirectiveRule for SuppressionWithoutReason {
    fn meta(&self) -> &'static RuleMeta {
        &SUPPRESSION_WITHOUT_REASON
    }

    fn check(&self, ctx: &DirectiveContext<'_>, sink: &mut FindingSink<'_>) {
        for directive in ctx.directives.iter().filter(|entry| entry.reason.is_none()) {
            sink.at(directive.span, "lint directive states no reason".to_owned());
        }
    }
}

/// Detects directives that silenced nothing.
pub struct UnusedSuppression;

/// Metadata for [`UnusedSuppression`].
static UNUSED_SUPPRESSION: RuleMeta = RuleMeta {
    name: "unused-suppression",
    category: Category::Suppression,
    stage: Stage::Directive,
    default_severity: DefaultSeverity::On(Severity::Advice),
    summary: "lint directive suppressed no finding",
    rationale: concat!(
        "A directive that suppresses nothing is usually left over from a ",
        "problem that has since been fixed. It then hides the next occurrence ",
        "of the same problem without anyone noticing."
    ),
    remediation: "Delete the directive, or narrow it to the rules it still needs to silence.",
};

impl DirectiveRule for UnusedSuppression {
    fn meta(&self) -> &'static RuleMeta {
        &UNUSED_SUPPRESSION
    }

    fn consumes_usage(&self) -> bool {
        true
    }

    /// Report directives that silenced nothing.
    ///
    /// The counts include the findings the other directive rules produced,
    /// because a directive that silenced one of those has done its job. They
    /// exclude this rule's own findings, which keeps it from depending on its
    /// own result.
    fn check(&self, ctx: &DirectiveContext<'_>, sink: &mut FindingSink<'_>) {
        for (directive, used) in ctx.directives.iter().zip(ctx.usage) {
            let names_known_rule = directive.rules.iter().any(|name| registry::is_known(name));
            if *used > 0 || !names_known_rule {
                continue;
            }
            sink.at(
                directive.span,
                format!(
                    "lint directive for {} suppressed no finding",
                    directive
                        .rules
                        .iter()
                        .map(|name| format!("`{name}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
        }
    }
}

#[cfg(test)]
#[path = "suppression_tests.rs"]
mod tests;
