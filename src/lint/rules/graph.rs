//! Rules about the lowered build graph.
//!
//! These bind to the graph stage because they need the resolved edge set: the
//! recipe text after interpolation, the outputs every edge claims, and
//! reachability from the manifest's defaults. None of that exists before
//! lowering.

use std::collections::BTreeSet;

use camino::Utf8PathBuf;

use crate::ast::Recipe;
use crate::ir::{BuildEdge, BuildGraph};
use crate::lint::registry::Registered;
use crate::lint::resolve::{self, Provenance};
use crate::lint::rule::{Category, FindingSink, GraphContext, GraphRule, RuleMeta, Stage};
use crate::lint::severity::{DefaultSeverity, Severity};

use super::shellscan;

/// Register this module's rules.
#[must_use]
pub fn rules() -> Vec<Registered> {
    vec![
        Registered::Graph(&UndeclaredTargetInput),
        Registered::Graph(&UnreachableTarget),
    ]
}

/// Detects a recipe consuming an output it has not declared a dependency on.
pub struct UndeclaredTargetInput;

/// Metadata for [`UndeclaredTargetInput`].
static UNDECLARED_TARGET_INPUT: RuleMeta = RuleMeta {
    name: "undeclared-target-input",
    category: Category::Correctness,
    stage: Stage::Graph,
    default_severity: DefaultSeverity::On(Severity::Warning),
    summary: "recipe names another target's output without depending on it",
    rationale: concat!(
        "Ninja schedules edges concurrently unless a dependency orders them. A ",
        "recipe that reads a path another target produces, without declaring ",
        "that path as a dependency, races: it succeeds whenever the producer ",
        "happened to run first, and fails or reads stale content when it did ",
        "not. Serial local builds hide this; a parallel or clean build does not."
    ),
    remediation: "Declare the path under `sources` if the recipe reads it, or under `deps` if it only needs it to exist.",
};

impl GraphRule for UndeclaredTargetInput {
    fn meta(&self) -> &'static RuleMeta {
        &UNDECLARED_TARGET_INPUT
    }

    /// Only paths the graph itself declares as outputs are searched for, and
    /// only on word boundaries, so ordinary command text cannot match.
    fn check(&self, ctx: &GraphContext<'_>, sink: &mut FindingSink<'_>) {
        let outputs = file_outputs(ctx.graph);
        let provenance = Provenance::new(ctx.document, ctx.manifest);
        for (index, target) in ctx.manifest.targets.iter().enumerate() {
            let Some(edge) = primary_edge(ctx.graph, target.name.to_string_vec().first()) else {
                continue;
            };
            let undeclared = undeclared_inputs(ctx, edge, &outputs);
            let span = resolve::node_span(provenance.target(index));
            let label = format!("target `{}`", target.name.to_string_vec().join(", "));
            for path in undeclared {
                sink.at_or_detached(
                    span,
                    label.clone(),
                    format!(
                        "recipe names `{path}`, which is built by another target, \
                         without declaring it as a dependency"
                    ),
                );
            }
        }
    }
}

/// Collect the graph outputs a recipe could plausibly read.
///
/// Phony outputs are excluded because they name no file: an action called
/// `test` or `install` would otherwise make every recipe that runs `test -f`
/// or `install -m` look like it consumed one. Very short names are excluded
/// for the same reason, since they match too much ordinary command text.
fn file_outputs(graph: &BuildGraph) -> Vec<&str> {
    graph
        .targets
        .iter()
        .filter(|(_, edge)| !edge.phony)
        .map(|(path, _)| path_str(path))
        .filter(|path| path.len() >= 3)
        .collect()
}

/// Report the graph outputs an edge's recipe names but does not depend on.
fn undeclared_inputs<'graph>(
    ctx: &GraphContext<'graph>,
    edge: &'graph BuildEdge,
    outputs: &[&'graph str],
) -> Vec<&'graph str> {
    let declared = dependency_closure(ctx.graph, edge);
    let texts = recipe_texts(ctx, &edge.action_id);
    outputs
        .iter()
        .filter(|path| !declared.contains(*path))
        .filter(|path| {
            texts
                .iter()
                .any(|text| !shellscan::find_words(text, path).is_empty())
        })
        .copied()
        .collect()
}

/// Borrow the edge that produces a target's first output.
fn primary_edge<'a>(graph: &'a BuildGraph, name: Option<&String>) -> Option<&'a BuildEdge> {
    graph.targets.get(Utf8PathBuf::from(name?).as_path())
}

/// Collect every path an edge is ordered after, transitively.
///
/// Transitivity matters: a recipe that names a path produced two edges away is
/// already ordered after it, so reporting it would be a false positive. The
/// edge's own outputs are included because naming them is what `{{ outs }}`
/// lowers to.
fn dependency_closure<'graph>(
    graph: &'graph BuildGraph,
    edge: &'graph BuildEdge,
) -> BTreeSet<&'graph str> {
    let mut reached: BTreeSet<&str> = edge
        .explicit_outputs
        .iter()
        .chain(&edge.implicit_outputs)
        .map(path_str)
        .collect();
    let mut queue: Vec<&str> = direct_dependencies(edge).collect();
    while let Some(path) = queue.pop() {
        if !reached.insert(path) {
            continue;
        }
        let Some(next) = graph.targets.get(Utf8PathBuf::from(path).as_path()) else {
            continue;
        };
        queue.extend(direct_dependencies(next));
    }
    reached
}

/// Iterate the paths an edge depends on directly, under any dependency key.
fn direct_dependencies(edge: &BuildEdge) -> impl Iterator<Item = &str> {
    edge.inputs
        .iter()
        .chain(&edge.implicit_deps)
        .chain(&edge.order_only_deps)
        .map(path_str)
}

/// Collect the shell text of the action an edge refers to.
fn recipe_texts(ctx: &GraphContext<'_>, action_id: &str) -> Vec<String> {
    let Some(action) = ctx.graph.actions.get(action_id) else {
        return Vec::new();
    };
    match &action.recipe {
        Recipe::Command { command } => command.to_string_vec(),
        Recipe::Script { script } => vec![script.clone()],
        Recipe::Rule { rule } => rule.to_string_vec(),
    }
}

/// Detects targets nothing reaches.
pub struct UnreachableTarget;

/// Metadata for [`UnreachableTarget`].
static UNREACHABLE_TARGET: RuleMeta = RuleMeta {
    name: "unreachable-target",
    category: Category::Clarity,
    stage: Stage::Graph,
    default_severity: DefaultSeverity::Off,
    summary: "target reachable from no default and no other target",
    rationale: concat!(
        "A target that no default lists and nothing depends on is only built ",
        "when someone names it on the command line. That is a legitimate ",
        "workflow, so this rule is off by default; a project that expects every ",
        "target to be reachable can enable it to catch the ones left behind by ",
        "a removed dependency."
    ),
    remediation: "Add the target to `defaults`, depend on it from a target that is reachable, or delete it.",
};

impl GraphRule for UnreachableTarget {
    /// This rule is off by default: building a target by name without
    /// declaring it a default is a supported workflow, so reachability is a
    /// project convention rather than a defect.
    fn meta(&self) -> &'static RuleMeta {
        &UNREACHABLE_TARGET
    }

    fn check(&self, ctx: &GraphContext<'_>, sink: &mut FindingSink<'_>) {
        let reachable = reachable_outputs(ctx.graph);
        let provenance = Provenance::new(ctx.document, ctx.manifest);
        for (index, target) in ctx.manifest.targets.iter().enumerate() {
            let outputs = target.name.to_string_vec();
            if outputs.is_empty() || outputs.iter().any(|name| reachable.contains(name.as_str())) {
                continue;
            }
            sink.at_or_detached(
                resolve::field_span(provenance.target(index), "name"),
                format!("target `{}`", outputs.join(", ")),
                "target is reachable from nothing".to_owned(),
            );
        }
    }
}

/// Collect every output reachable from the graph's defaults.
///
/// A graph with no defaults treats every output as reachable, because
/// `netsuke build` then requires an explicit target and every target is an
/// entry point.
fn reachable_outputs(graph: &BuildGraph) -> BTreeSet<&str> {
    let mut reachable: BTreeSet<&str> = BTreeSet::new();
    if graph.default_targets.is_empty() {
        return graph.targets.keys().map(path_str).collect();
    }
    let mut queue: Vec<&str> = graph.default_targets.iter().map(path_str).collect();
    while let Some(path) = queue.pop() {
        if !reachable.insert(path) {
            continue;
        }
        let Some(edge) = graph.targets.get(Utf8PathBuf::from(path).as_path()) else {
            continue;
        };
        queue.extend(
            edge.inputs
                .iter()
                .chain(&edge.implicit_deps)
                .chain(&edge.order_only_deps)
                .map(path_str),
        );
    }
    reachable
}

/// Borrow a graph path as a string slice.
fn path_str(path: &Utf8PathBuf) -> &str {
    path.as_str()
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
