//! Rules about declarations nothing uses.
//!
//! Unused-variable and unused-macro analysis binds to the document stage
//! because rendering consumes the very references it needs: by the time the
//! manifest is typed, `{{ cc }}` has become `gcc` and the reference to `cc`
//! has gone. Unused-rule analysis binds to the manifest stage instead, because
//! a `foreach` target's rule reference only exists once the loop is unrolled.

use std::collections::BTreeSet;

use crate::ast::{Recipe, StringOrList};
use crate::lint::document::{Document, Node, NodeKind, Span};
use crate::lint::registry::Registered;
use crate::lint::resolve::{self, Provenance};
use crate::lint::rule::{
    Category, DocumentRule, FindingSink, ManifestContext, ManifestRule, RuleMeta, Stage,
};
use crate::lint::severity::{DefaultSeverity, Severity};

/// Register this module's rules.
#[must_use]
pub fn rules() -> Vec<Registered> {
    vec![
        Registered::Document(&UnusedVar),
        Registered::Document(&UnusedMacro),
        Registered::Manifest(&UnusedRule),
    ]
}

/// Detects global variables no template references.
pub struct UnusedVar;

/// Metadata for [`UnusedVar`].
static UNUSED_VAR: RuleMeta = RuleMeta {
    name: "unused-var",
    category: Category::Hygiene,
    stage: Stage::Document,
    default_severity: DefaultSeverity::On(Severity::Warning),
    summary: "global `vars` entry that no template references",
    rationale: concat!(
        "An unreferenced variable is usually a rename that was not finished or ",
        "a recipe that stopped using it. Either way a reader has to work out ",
        "whether it still matters, and a later edit that reintroduces the name ",
        "silently picks up a stale value."
    ),
    remediation: "Delete the entry, or reference it from the recipe that was meant to use it.",
};

impl DocumentRule for UnusedVar {
    fn meta(&self) -> &'static RuleMeta {
        &UNUSED_VAR
    }

    fn check(&self, doc: &Document, sink: &mut FindingSink<'_>) {
        let referenced = referenced_identifiers(doc);
        let Some(vars) = doc.section("vars").and_then(Node::as_mapping) else {
            return;
        };
        let declarations = vars
            .iter()
            .filter_map(|entry| Some((entry.key.as_str()?, entry.key.span)));
        report_unreferenced_declarations(declarations, &referenced, sink, |name| {
            format!("global variable `{name}` is never referenced")
        });
    }
}

/// Detects macros nothing calls.
pub struct UnusedMacro;

/// Metadata for [`UnusedMacro`].
static UNUSED_MACRO: RuleMeta = RuleMeta {
    name: "unused-macro",
    category: Category::Hygiene,
    stage: Stage::Document,
    default_severity: DefaultSeverity::On(Severity::Warning),
    summary: "declared macro that nothing calls",
    rationale: concat!(
        "A macro is registered before any other field renders, so an unused one ",
        "still costs a reader the effort of understanding it and still occupies ",
        "the template namespace that variables and helpers share."
    ),
    remediation: "Delete the macro, or call it from the field it was written for.",
};

impl DocumentRule for UnusedMacro {
    fn meta(&self) -> &'static RuleMeta {
        &UNUSED_MACRO
    }

    fn check(&self, doc: &Document, sink: &mut FindingSink<'_>) {
        let referenced = referenced_identifiers(doc);
        let Some(macros) = doc.section("macros") else {
            return;
        };
        let declarations = macros.items().filter_map(|item| {
            let signature = item.get("signature");
            let name = signature.and_then(Node::as_str).map(macro_name)?;
            Some((name, signature.map_or(item.span, |node| node.span)))
        });
        report_unreferenced_declarations(declarations, &referenced, sink, |name| {
            format!("macro `{name}` is never called")
        });
    }
}

/// Report every declaration whose name no template mentions.
///
/// The unused-variable and unused-macro rules differ only in where the
/// declarations come from and how the finding reads. Sharing the loop keeps
/// the decision they have in common — that a name absent from the reference
/// set is unused — in one place, so the two cannot answer it differently.
fn report_unreferenced_declarations<'a>(
    declarations: impl IntoIterator<Item = (&'a str, Span)>,
    referenced: &BTreeSet<String>,
    sink: &mut FindingSink<'_>,
    message: impl Fn(&str) -> String,
) {
    let unreferenced = declarations
        .into_iter()
        .filter(|(name, _)| !referenced.contains(*name));
    for (name, span) in unreferenced {
        sink.at(span, message(name));
    }
}

/// Extract a macro's name from its signature.
fn macro_name(signature: &str) -> &str {
    signature
        .split_once('(')
        .map_or(signature, |(name, _)| name)
        .trim()
}

/// Collect every identifier any template expression mentions.
///
/// The scan is deliberately generous: it collects identifiers from every
/// scalar in the document, including `foreach` and `when` values, which are
/// bare expressions rather than delimited templates. A rule that reports an
/// unused declaration must not fire on one that is used, so over-collecting is
/// the safe direction.
fn referenced_identifiers(doc: &Document) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(root) = doc.root() else {
        return names;
    };
    let declarations = declaration_spans(doc);
    root.walk(&mut |node: &Node| {
        let NodeKind::Scalar { value, .. } = &node.kind else {
            return;
        };
        if declarations.contains(&node.span) {
            return;
        }
        collect_identifiers(value, &mut names);
    });
    names
}

/// Collect the spans of the scalars that declare the names under analysis.
///
/// A `vars` key and a macro signature both spell the name they define, so
/// counting them as references would make every declaration look used.
fn declaration_spans(doc: &Document) -> BTreeSet<Span> {
    let mut spans = BTreeSet::new();
    if let Some(vars) = doc.section("vars").and_then(Node::as_mapping) {
        spans.extend(vars.iter().map(|entry| entry.key.span));
    }
    if let Some(macros) = doc.section("macros") {
        spans.extend(
            macros
                .items()
                .filter_map(|item| item.get("signature"))
                .map(|node| node.span),
        );
    }
    spans
}

/// Collect identifier-shaped words from one scalar.
fn collect_identifiers(text: &str, names: &mut BTreeSet<String>) {
    let mut current = String::new();
    for character in text.chars() {
        if character.is_alphanumeric() || character == '_' {
            current.push(character);
            continue;
        }
        if !current.is_empty() {
            names.insert(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        names.insert(current);
    }
}

/// Detects rules no target or action references.
pub struct UnusedRule;

/// Metadata for [`UnusedRule`].
static UNUSED_RULE: RuleMeta = RuleMeta {
    name: "unused-rule",
    category: Category::Hygiene,
    stage: Stage::Manifest,
    default_severity: DefaultSeverity::On(Severity::Warning),
    summary: "declared rule that no target or action references",
    rationale: concat!(
        "A rule exists to be shared. One that nothing references contributes no ",
        "build edge, so it is either dead weight left by a removed target or a ",
        "rule whose name a target misspells."
    ),
    remediation: "Delete the rule, or point the target that should share it at the rule's name.",
};

impl ManifestRule for UnusedRule {
    fn meta(&self) -> &'static RuleMeta {
        &UNUSED_RULE
    }

    /// Reference collection runs over the expanded manifest so that a rule
    /// referenced only by a `foreach`-generated target counts as used.
    fn check(&self, ctx: &ManifestContext<'_>, sink: &mut FindingSink<'_>) {
        let referenced = referenced_rules(ctx);
        let provenance = Provenance::new(ctx.document, ctx.manifest);
        for (index, rule) in ctx.manifest.rules.iter().enumerate() {
            if referenced.contains(rule.name.as_str()) {
                continue;
            }
            sink.at_or_detached(
                resolve::field_span(provenance.rule(index), "name"),
                format!("rule `{}`", rule.name),
                "declared rule is never referenced".to_owned(),
            );
        }
    }
}

/// Collect every rule name a target, action, or other rule references.
fn referenced_rules<'a>(ctx: &'a ManifestContext<'a>) -> BTreeSet<&'a str> {
    let recipes = ctx
        .manifest
        .targets
        .iter()
        .chain(&ctx.manifest.actions)
        .map(|target| &target.recipe)
        .chain(ctx.manifest.rules.iter().map(|rule| &rule.recipe));
    recipes.flat_map(rule_references).collect()
}

/// Report the rule names one recipe references.
fn rule_references(recipe: &Recipe) -> Vec<&str> {
    match recipe {
        Recipe::Rule { rule } => match rule {
            StringOrList::Empty => Vec::new(),
            StringOrList::String(name) => vec![name.as_str()],
            StringOrList::List(names) => names.iter().map(String::as_str).collect(),
        },
        Recipe::Command { .. } | Recipe::Script { .. } => Vec::new(),
    }
}

#[cfg(test)]
#[path = "hygiene_tests.rs"]
mod tests;
