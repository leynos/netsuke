//! Ninja file generator.
//!
//! This module converts a [`crate::ir::BuildGraph`] into the textual
//! representation expected by the Ninja build system. The generator sorts
//! actions and edges to ensure deterministic output for snapshot tests.

use crate::ast::Recipe;
use crate::ir::{BuildEdge, BuildGraph};
use itertools::Itertools;
use std::collections::HashSet;
use std::fmt::{self, Display, Formatter, Write};
use std::path::PathBuf;

/// Generate a Ninja build file as a string.
///
/// # Panics
///
/// Panics if a build edge references an unknown action.
#[must_use]
pub fn generate(graph: &BuildGraph) -> String {
    let mut out = String::new();

    let mut actions: Vec<_> = graph.actions.iter().collect();
    actions.sort_by_key(|(id, _)| *id);
    for (id, action) in actions {
        write!(out, "{}", NamedAction { id, action }).expect("write Ninja rule");
    }

    let mut edges: Vec<_> = graph.targets.values().collect();
    edges.sort_by(|a, b| path_key(&a.explicit_outputs).cmp(&path_key(&b.explicit_outputs)));
    let mut seen = HashSet::new();
    for edge in edges {
        let key = path_key(&edge.explicit_outputs);
        if !seen.insert(key.clone()) {
            continue;
        }
        let action = graph.actions.get(&edge.action_id).expect("action");
        write!(
            out,
            "{}",
            DisplayEdge {
                edge,
                action_restat: action.restat,
            }
        )
        .expect("write Ninja edge");
    }

    if !graph.default_targets.is_empty() {
        let mut defs = graph.default_targets.clone();
        defs.sort();
        writeln!(out, "default {}", join(&defs)).expect("write defaults");
    }

    out
}

/// Convert a slice of paths into a space-separated string.
fn join(paths: &[PathBuf]) -> String {
    paths.iter().map(|p| p.display()).join(" ")
}

/// Generate a stable key for a list of paths.
fn path_key(paths: &[PathBuf]) -> String {
    let mut parts: Vec<_> = paths.iter().map(|p| p.display().to_string()).collect();
    parts.sort();
    parts.join("\u{0}")
}

/// Wrapper struct to display a rule with its identifier.
struct NamedAction<'a> {
    id: &'a str,
    action: &'a crate::ir::Action,
}

impl Display for NamedAction<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "rule {}", self.id)?;
        match &self.action.recipe {
            Recipe::Command { command } => writeln!(f, "  command = {command}")?,
            Recipe::Script { script } => {
                writeln!(f, "  command = /bin/sh -e -c \"")?;
                for line in script.lines() {
                    writeln!(f, "    {line}")?;
                }
                writeln!(f, "  \"")?;
            }
            Recipe::Rule { .. } => unreachable!("rules do not reference other rules"),
        }
        if let Some(desc) = &self.action.description {
            writeln!(f, "  description = {desc}")?;
        }
        if let Some(depfile) = &self.action.depfile {
            writeln!(f, "  depfile = {depfile}")?;
        }
        if let Some(deps_format) = &self.action.deps_format {
            writeln!(f, "  deps = {deps_format}")?;
        }
        if let Some(pool) = &self.action.pool {
            writeln!(f, "  pool = {pool}")?;
        }
        if self.action.restat {
            writeln!(f, "  restat = 1")?;
        }
        writeln!(f)
    }
}

/// Wrapper struct to display a build edge.
struct DisplayEdge<'a> {
    edge: &'a BuildEdge,
    action_restat: bool,
}

impl Display for DisplayEdge<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "build {}", join(&self.edge.explicit_outputs))?;
        if !self.edge.implicit_outputs.is_empty() {
            write!(f, " | {}", join(&self.edge.implicit_outputs))?;
        }
        let rule = if self.edge.phony {
            "phony"
        } else {
            &self.edge.action_id
        };
        write!(f, ": {rule}")?;
        if !self.edge.inputs.is_empty() {
            write!(f, " {}", join(&self.edge.inputs))?;
        }
        if !self.edge.order_only_deps.is_empty() {
            write!(f, " || {}", join(&self.edge.order_only_deps))?;
        }
        writeln!(f)?;
        if self.edge.always && !self.action_restat {
            writeln!(f, "  restat = 1")?;
        }
        writeln!(f)
    }
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
