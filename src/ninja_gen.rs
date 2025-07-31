//! Ninja file generator.
//!
//! This module converts a [`crate::ir::BuildGraph`] into the textual
//! representation expected by the Ninja build system.
//! The output is deterministic: actions and edges are sorted to ensure
//! stable snapshots for testing.

use crate::ast::Recipe;
use crate::ir::{BuildEdge, BuildGraph};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::path::PathBuf;

/// Generate a Ninja build file as a string.
#[must_use]
pub fn generate(graph: &BuildGraph) -> String {
    let mut out = String::new();
    write_rules(&mut out, &graph.actions);
    write_edges(&mut out, &graph.targets);
    write_defaults(&mut out, &graph.default_targets);
    out
}

fn write_rules(out: &mut String, actions: &HashMap<String, crate::ir::Action>) {
    let mut ids: Vec<_> = actions.keys().collect();
    ids.sort();
    for id in ids {
        let action = &actions[id];
        let _ = writeln!(out, "rule {id}");
        match &action.recipe {
            Recipe::Command { command } => {
                let _ = writeln!(out, "  command = {command}");
            }
            Recipe::Script { script } => {
                let _ = writeln!(out, "  command = /bin/sh -e -c \"");
                for line in script.lines() {
                    let _ = writeln!(out, "    {line}");
                }
                let _ = writeln!(out, "  \"");
            }
            Recipe::Rule { .. } => unreachable!("rules do not reference other rules"),
        }
        if let Some(desc) = &action.description {
            let _ = writeln!(out, "  description = {desc}");
        }
        if let Some(depfile) = &action.depfile {
            let _ = writeln!(out, "  depfile = {depfile}");
        }
        if let Some(deps_format) = &action.deps_format {
            let _ = writeln!(out, "  deps = {deps_format}");
        }
        if let Some(pool) = &action.pool {
            let _ = writeln!(out, "  pool = {pool}");
        }
        if action.restat {
            let _ = writeln!(out, "  restat = 1");
        }
        let _ = writeln!(out);
    }
}

fn write_edges(out: &mut String, targets: &HashMap<PathBuf, BuildEdge>) {
    let mut edges: Vec<&BuildEdge> = targets.values().collect();
    edges.sort_by(|a, b| a.explicit_outputs.cmp(&b.explicit_outputs));
    let mut seen = HashSet::new();
    for edge in edges {
        if edge
            .explicit_outputs
            .first()
            .is_some_and(|f| !seen.insert(f))
        {
            continue;
        }
        write_edge(out, edge);
    }
}

fn write_edge(out: &mut String, edge: &BuildEdge) {
    let outputs = join(&edge.explicit_outputs);
    let _ = write!(out, "build {outputs}");
    if !edge.implicit_outputs.is_empty() {
        let _ = write!(out, " | {}", join(&edge.implicit_outputs));
    }
    let rule = if edge.phony { "phony" } else { &edge.action_id };
    let _ = write!(out, ": {rule}");
    if !edge.inputs.is_empty() {
        let _ = write!(out, " {}", join(&edge.inputs));
    }
    if !edge.order_only_deps.is_empty() {
        let _ = write!(out, " || {}", join(&edge.order_only_deps));
    }
    let _ = writeln!(out);
    if edge.always {
        let _ = writeln!(out, "  restat = 1");
    }
    let _ = writeln!(out);
}

fn write_defaults(out: &mut String, defaults: &[PathBuf]) {
    if defaults.is_empty() {
        return;
    }
    let mut defs: Vec<_> = defaults.iter().collect();
    defs.sort();
    let _ = writeln!(
        out,
        "default {}",
        defs.iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );
}

fn join(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Action, BuildEdge, BuildGraph};
    use rstest::rstest;

    #[rstest]
    fn generate_simple_ninja() {
        let action = Action {
            recipe: Recipe::Command {
                command: "echo hi".into(),
            },
            description: None,
            depfile: None,
            deps_format: None,
            pool: None,
            restat: false,
        };
        let edge = BuildEdge {
            action_id: "a".into(),
            inputs: vec![PathBuf::from("in")],
            explicit_outputs: vec![PathBuf::from("out")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
        };
        let mut graph = BuildGraph::default();
        graph.actions.insert("a".into(), action);
        graph.targets.insert(PathBuf::from("out"), edge);
        graph.default_targets.push(PathBuf::from("out"));

        let ninja = generate(&graph);
        let expected = concat!(
            "rule a\n",
            "  command = echo hi\n\n",
            "build out: a in\n\n",
            "default out\n"
        );
        assert_eq!(ninja, expected);
    }
}
