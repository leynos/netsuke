//! Rules about individual declarations that have no effect.

use crate::lint::document::{Document, Node};
use crate::lint::registry::Registered;
use crate::lint::rule::{Category, DocumentRule, FindingSink, RuleMeta, Stage};
use crate::lint::severity::{DefaultSeverity, Severity};

use crate::lint::rules::recipes::{self, Item, Section};

/// Register this module's rules.
#[must_use]
pub fn rules() -> Vec<Registered> {
    vec![
        Registered::Document(&BuiltinCleanAction),
        Registered::Document(&SerialOrderWithoutDeps),
        Registered::Document(&RedundantAlways),
    ]
}

/// Detects a handwritten `clean` action.
pub struct BuiltinCleanAction;

/// Metadata for [`BuiltinCleanAction`].
static BUILTIN_CLEAN_ACTION: RuleMeta = RuleMeta {
    name: "builtin-clean-action",
    category: Category::Redundancy,
    stage: Stage::Document,
    default_severity: DefaultSeverity::On(Severity::Advice),
    summary: "action named `clean` duplicates the built-in `netsuke clean`",
    rationale: concat!(
        "`netsuke clean` removes exactly the outputs the graph declares, by ",
        "asking Ninja. A handwritten `clean` action removes whatever its ",
        "recipe names, which drifts from the graph as targets are added and ",
        "typically reaches for a wildcard that can delete more than it should."
    ),
    remediation: "Delete the action and use `netsuke clean`, which is derived from the graph.",
};

impl DocumentRule for BuiltinCleanAction {
    fn meta(&self) -> &'static RuleMeta {
        &BUILTIN_CLEAN_ACTION
    }

    fn check(&self, doc: &Document, sink: &mut FindingSink<'_>) {
        for item in recipes::items(doc).into_iter().filter(is_clean_action) {
            sink.at(
                item.field_span("name"),
                "action `clean` duplicates the built-in `netsuke clean`".to_owned(),
            );
        }
    }
}

/// Report whether an item is an action named `clean`.
fn is_clean_action(item: &Item<'_>) -> bool {
    item.section == Section::Actions && item.name().is_some_and(|name| name == "clean")
}

/// Detects serial ordering declared on a list too short to order.
pub struct SerialOrderWithoutDeps;

/// Metadata for [`SerialOrderWithoutDeps`].
static SERIAL_ORDER_WITHOUT_DEPS: RuleMeta = RuleMeta {
    name: "serial-order-without-deps",
    category: Category::Redundancy,
    stage: Stage::Document,
    default_severity: DefaultSeverity::On(Severity::Advice),
    summary: "`dependency_order: serial` declared with fewer than two `deps`",
    rationale: concat!(
        "Serial ordering sequences the entries of a `deps` list. With no ",
        "dependencies, or one, there is nothing to sequence, so the declaration ",
        "has no effect. It usually means the dependencies were meant to be ",
        "listed under `deps` and were written under `sources` instead."
    ),
    remediation: "List the ordered work under `deps`, or remove `dependency_order`.",
};

impl DocumentRule for SerialOrderWithoutDeps {
    fn meta(&self) -> &'static RuleMeta {
        &SERIAL_ORDER_WITHOUT_DEPS
    }

    fn check(&self, doc: &Document, sink: &mut FindingSink<'_>) {
        for item in recipes::items(doc) {
            let is_serial = item
                .node
                .get("dependency_order")
                .and_then(Node::as_str)
                .is_some_and(|value| value == "serial");
            if !is_serial || entry_count(item.node.get("deps")) >= 2 {
                continue;
            }
            sink.at(
                item.field_span("dependency_order"),
                format!(
                    "{} declares serial ordering over {} direct dependencies",
                    item.label(),
                    entry_count(item.node.get("deps"))
                ),
            );
        }
    }
}

/// Count the entries of a node that may be absent, a scalar, or a list.
fn entry_count(node: Option<&Node>) -> usize {
    node.map_or(0, |found| {
        found.as_str().map_or_else(|| found.items().count(), |_| 1)
    })
}

/// Detects `always` on a target that is already phony.
pub struct RedundantAlways;

/// Metadata for [`RedundantAlways`].
static REDUNDANT_ALWAYS: RuleMeta = RuleMeta {
    name: "redundant-always",
    category: Category::Redundancy,
    stage: Stage::Document,
    default_severity: DefaultSeverity::On(Severity::Advice),
    summary: "`always` declared on a target that is already phony",
    rationale: concat!(
        "A phony target is always considered out of date, and every action is ",
        "implicitly phony. Adding `always` to one states the same thing twice ",
        "and suggests the author expected it to mean something more."
    ),
    remediation: "Remove `always`; the target already runs whenever it is requested.",
};

impl DocumentRule for RedundantAlways {
    fn meta(&self) -> &'static RuleMeta {
        &REDUNDANT_ALWAYS
    }

    fn check(&self, doc: &Document, sink: &mut FindingSink<'_>) {
        for item in recipes::items(doc) {
            if !is_flag_set(&item, "always") || !is_phony(&item) {
                continue;
            }
            sink.at(
                item.field_span("always"),
                format!("{} is phony, so `always` adds nothing", item.label()),
            );
        }
    }
}

/// Report whether a Boolean manifest flag is set on an item.
fn is_flag_set(item: &Item<'_>, field: &str) -> bool {
    item.node
        .get(field)
        .and_then(Node::as_bool)
        .unwrap_or_default()
}

/// Report whether an item is phony, explicitly or by being an action.
fn is_phony(item: &Item<'_>) -> bool {
    item.section == Section::Actions || is_flag_set(item, "phony")
}

#[cfg(test)]
#[path = "declarations_tests.rs"]
mod tests;
