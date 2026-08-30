//! Rules about the same thing declared twice.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use crate::ast::{Recipe, StringOrList, Target};
use crate::lint::document::Node;
use crate::lint::registry::Registered;
use crate::lint::resolve::{self, Provenance};
use crate::lint::rule::{Category, FindingSink, ManifestContext, ManifestRule, RuleMeta, Stage};
use crate::lint::severity::{DefaultSeverity, Severity};

/// Register this module's rules.
#[must_use]
pub fn rules() -> Vec<Registered> {
    vec![
        Registered::Manifest(&DuplicateRuleRecipe),
        Registered::Manifest(&RedundantDependency),
    ]
}

/// Detects two rules with identical recipes.
pub struct DuplicateRuleRecipe;

/// Metadata for [`DuplicateRuleRecipe`].
static DUPLICATE_RULE_RECIPE: RuleMeta = RuleMeta {
    name: "duplicate-rule-recipe",
    category: Category::Redundancy,
    stage: Stage::Manifest,
    default_severity: DefaultSeverity::On(Severity::Warning),
    summary: "two rules declare identical recipes",
    rationale: concat!(
        "Two rules with the same recipe are one rule under two names. A change ",
        "to the shared command has to be made twice, and a reader cannot tell ",
        "which name to use for new targets. Netsuke already deduplicates the ",
        "generated action, so the duplication buys nothing at build time."
    ),
    remediation: "Keep one rule and point the other rule's targets at it.",
};

impl ManifestRule for DuplicateRuleRecipe {
    fn meta(&self) -> &'static RuleMeta {
        &DUPLICATE_RULE_RECIPE
    }

    /// Compare recipes after expansion, so two rules that render to the same
    /// command are reported even when their templates differ.
    fn check(&self, ctx: &ManifestContext<'_>, sink: &mut FindingSink<'_>) {
        let provenance = Provenance::new(ctx.document, ctx.manifest);
        let mut seen: BTreeMap<String, &str> = BTreeMap::new();
        for (index, rule) in ctx.manifest.rules.iter().enumerate() {
            let Entry::Occupied(slot) = seen.entry(recipe_key(&rule.recipe)) else {
                seen.insert(recipe_key(&rule.recipe), rule.name.as_str());
                continue;
            };
            let first = *slot.get();
            sink.at_or_detached(
                resolve::field_span(provenance.rule(index), "name"),
                format!("rule `{}`", rule.name),
                format!("declares the same recipe as rule `{first}`"),
            );
        }
    }
}

/// Build a canonical key that two identical recipes share.
///
/// The key names the recipe kind so that a `command` and a `script` holding
/// the same text are not mistaken for duplicates.
fn recipe_key(recipe: &Recipe) -> String {
    match recipe {
        Recipe::Command { command } => format!("command:{:?}", command.to_string_vec()),
        Recipe::Script { script } => format!("script:{script}"),
        Recipe::Rule { rule } => format!("rule:{:?}", rule.to_string_vec()),
    }
}

/// Detects a path declared under more than one dependency key.
pub struct RedundantDependency;

/// Metadata for [`RedundantDependency`].
static REDUNDANT_DEPENDENCY: RuleMeta = RuleMeta {
    name: "redundant-dependency",
    category: Category::Redundancy,
    stage: Stage::Manifest,
    default_severity: DefaultSeverity::On(Severity::Advice),
    summary: "path declared under more than one dependency key",
    rationale: concat!(
        "`sources`, `deps`, and `order_only_deps` are ordered by strength: a ",
        "source rebuilds the target and becomes `{{ ins }}`, an implicit ",
        "dependency rebuilds it, and an order-only dependency only sequences ",
        "it. Declaring one path under two keys leaves the weaker declaration ",
        "with no effect, and hides which behaviour the author wanted."
    ),
    remediation: "Keep the strongest declaration the target needs and delete the other.",
};

impl ManifestRule for RedundantDependency {
    fn meta(&self) -> &'static RuleMeta {
        &REDUNDANT_DEPENDENCY
    }

    fn check(&self, ctx: &ManifestContext<'_>, sink: &mut FindingSink<'_>) {
        let provenance = Provenance::new(ctx.document, ctx.manifest);
        let sections = [
            (&ctx.manifest.actions, true),
            (&ctx.manifest.targets, false),
        ];
        for (targets, is_action) in sections {
            report_section(targets, &provenance, is_action, sink);
        }
    }
}

/// Report every duplicated dependency in one manifest section.
fn report_section(
    targets: &[Target],
    provenance: &Provenance<'_>,
    is_action: bool,
    sink: &mut FindingSink<'_>,
) {
    for (index, target) in targets.iter().enumerate() {
        let node = if is_action {
            provenance.action(index)
        } else {
            provenance.target(index)
        };
        report_target(target, node, sink);
    }
}

/// Report every path one target declares under more than one key.
///
/// Keys are visited from strongest to weakest, so the first declaration a path
/// receives is the one the target actually relies on and the later one is the
/// redundancy reported.
fn report_target(target: &Target, node: Option<&Node>, sink: &mut FindingSink<'_>) {
    let lists = [
        ("sources", &target.sources),
        ("deps", &target.deps),
        ("order_only_deps", &target.order_only_deps),
    ];
    let mut owner: BTreeMap<&str, &'static str> = BTreeMap::new();
    let duplicates = lists
        .into_iter()
        .flat_map(|(key, list)| entries(list).into_iter().map(move |path| (key, path)))
        .filter_map(|(key, path)| match owner.entry(path) {
            Entry::Vacant(slot) => {
                slot.insert(key);
                None
            }
            Entry::Occupied(slot) => Some((key, path, *slot.get())),
        });
    for (key, path, first) in duplicates {
        sink.at_or_detached(
            resolve::entry_span(node, key, path),
            target_label(target),
            format!("declares `{path}` under both `{first}` and `{key}`"),
        );
    }
}

/// Borrow the entries of a `StringOrList` without allocating.
fn entries(list: &StringOrList) -> Vec<&str> {
    match list {
        StringOrList::Empty => Vec::new(),
        StringOrList::String(value) => vec![value.as_str()],
        StringOrList::List(values) => values.iter().map(String::as_str).collect(),
    }
}

/// Name a target for diagnostics that cannot resolve a span.
fn target_label(target: &Target) -> String {
    let name = match &target.name {
        StringOrList::String(name) => name.clone(),
        StringOrList::List(names) => names.first().cloned().unwrap_or_default(),
        StringOrList::Empty => String::new(),
    };
    format!("target `{name}`")
}

#[cfg(test)]
#[path = "duplication_tests.rs"]
mod tests;
