//! Rules about how a recipe is written rather than what it does.

use crate::lint::document::{Document, Node};
use crate::lint::registry::Registered;
use crate::lint::rule::{Category, DocumentRule, FindingSink, RuleMeta, Stage};
use crate::lint::severity::{DefaultSeverity, Severity};

use crate::lint::rules::recipes::{self, Item, RecipePart, Section};
use crate::lint::rules::shellscan;

/// Register this module's rules.
#[must_use]
pub fn rules() -> Vec<Registered> {
    vec![
        Registered::Document(&LiteralRecipePath),
        Registered::Document(&CommandChainNotList),
    ]
}

/// Detects recipes that repeat a path the target already declares.
pub struct LiteralRecipePath;

/// Metadata for [`LiteralRecipePath`].
static LITERAL_RECIPE_PATH: RuleMeta = RuleMeta {
    name: "literal-recipe-path",
    category: Category::Clarity,
    stage: Stage::Document,
    default_severity: DefaultSeverity::On(Severity::Warning),
    summary: "recipe repeats a path the target already declares",
    rationale: concat!(
        "Netsuke substitutes and shell-quotes the declared inputs and outputs ",
        "through `{{ ins }}` and `{{ outs }}`. A recipe that spells the same ",
        "path out again states it twice: renaming the target, adding an output, ",
        "or generating the target with `foreach` then changes one copy and not ",
        "the other, and the literal copy is not shell-quoted."
    ),
    remediation: "Replace the literal path with `{{ outs }}` for outputs or `{{ ins }}` for sources.",
};

impl DocumentRule for LiteralRecipePath {
    fn meta(&self) -> &'static RuleMeta {
        &LITERAL_RECIPE_PATH
    }

    fn check(&self, doc: &Document, sink: &mut FindingSink<'_>) {
        for item in recipes::items(doc).into_iter().filter(declares_files) {
            let declared = declared_paths(&item);
            for part in recipes::item_parts(doc, item) {
                report_repeats(&part, &declared, sink);
            }
        }
    }
}

/// Report every declared path one recipe fragment spells out again.
fn report_repeats(
    part: &RecipePart<'_>,
    declared: &[(String, &'static str)],
    sink: &mut FindingSink<'_>,
) {
    let repeats = declared.iter().flat_map(|(path, placeholder)| {
        shellscan::find_words(part.source, path)
            .into_iter()
            .map(move |found| (found, path, placeholder))
    });
    for (found, path, placeholder) in repeats {
        sink.at(
            part.sub_span(found.start, found.len),
            format!(
                "{} `{}` repeats the declared path `{path}`; use `{placeholder}`",
                part.item.label(),
                part.kind.key()
            ),
        );
    }
}

/// Report whether an item's `name` and `sources` are file paths.
///
/// A rule's `name` identifies the rule, and an action's identifies a phony
/// operation; neither is a path a recipe could sensibly repeat. Only
/// file-producing targets are compared.
fn declares_files(item: &Item<'_>) -> bool {
    item.section == Section::Targets
        && item
            .node
            .get("phony")
            .and_then(Node::as_str)
            .is_none_or(|value| value != "true")
}

/// Collect the literal paths an item declares, paired with their placeholder.
///
/// Only literal scalars are collected: a templated `name` is not a path the
/// author could have written verbatim in the recipe, and a very short path
/// would match too much ordinary command text.
fn declared_paths(item: &Item<'_>) -> Vec<(String, &'static str)> {
    [("name", "{{ outs }}"), ("sources", "{{ ins }}")]
        .into_iter()
        .flat_map(|(field, placeholder)| {
            scalars(item.node.get(field))
                .into_iter()
                .map(move |path| (path, placeholder))
        })
        .filter(|(path, _)| is_matchable_path(path))
        .collect()
}

/// Collect the literal scalar values of a node that may be a scalar or a list.
fn scalars(node: Option<&Node>) -> Vec<String> {
    let Some(found) = node else {
        return Vec::new();
    };
    found.as_str().map_or_else(
        || {
            found
                .items()
                .filter_map(Node::as_str)
                .map(str::to_owned)
                .collect()
        },
        |value| vec![value.to_owned()],
    )
}

/// Report whether a declared path is specific enough to match on.
fn is_matchable_path(path: &str) -> bool {
    path.len() >= 3
        && !path.contains("{{")
        && !path.contains("{%")
        && path.chars().any(char::is_alphabetic)
}

/// Detects scalar commands that chain with `&&`.
pub struct CommandChainNotList;

/// Metadata for [`CommandChainNotList`].
static COMMAND_CHAIN_NOT_LIST: RuleMeta = RuleMeta {
    name: "command-chain-not-list",
    category: Category::Clarity,
    stage: Stage::Document,
    default_severity: DefaultSeverity::On(Severity::Advice),
    summary: "scalar `command` chains steps with `&&`",
    rationale: concat!(
        "A `command` list runs its entries in declaration order and stops at ",
        "the first non-zero exit, which is what the `&&` chain is emulating. ",
        "The list form reads as one step per line and reports which entry ",
        "failed by position, where the chained form reports only that the ",
        "whole command failed."
    ),
    remediation: "Write the steps as a YAML list under `command`, one entry per step.",
};

impl DocumentRule for CommandChainNotList {
    fn meta(&self) -> &'static RuleMeta {
        &COMMAND_CHAIN_NOT_LIST
    }

    fn check(&self, doc: &Document, sink: &mut FindingSink<'_>) {
        let scalars = recipes::parts(doc)
            .into_iter()
            .filter(|part| part.kind == recipes::RecipeKind::Command && !part.is_list_entry);
        for part in scalars {
            let chained = shellscan::find_all(part.source, "&&").len();
            if chained == 0 {
                continue;
            }
            sink.at(
                part.span,
                format!(
                    "{} chains {} steps in one `command` scalar",
                    part.item.label(),
                    chained.saturating_add(1)
                ),
            );
        }
    }
}

#[cfg(test)]
#[path = "recipe_shape_tests.rs"]
mod tests;
