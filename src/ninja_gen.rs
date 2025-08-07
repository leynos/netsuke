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

macro_rules! write_kv {
    ($f:expr, $key:expr, $opt:expr) => {
        if let Some(val) = $opt {
            writeln!($f, "  {} = {}", $key, val)?;
        }
    };
}

macro_rules! write_flag {
    ($f:expr, $key:expr, $cond:expr) => {
        if $cond {
            writeln!($f, "  {} = 1", $key)?;
        }
    };
}

/// Generate a Ninja build file as a string.
///
/// # Panics
///
/// Panics if a build edge references an unknown action or if writing to the
/// output string fails (which is unexpected under normal conditions).
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

/// Escape a script for embedding within a single-quoted `printf %b` argument.
///
/// Backslashes, dollar signs, double quotes, backticks, and single quotes are
/// escaped so the outer shell preserves them, while newlines become `\n` to
/// keep the rule on one line. Percent signs are passed through unchanged because
/// the script is an argument rather than a format string, allowing the inner
/// shell to perform variable expansion.
fn escape_script(script: &str) -> String {
    script
        .replace('\\', "\\\\")
        .replace('$', "\\$")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('\'', "'\"'\"'")
        .replace('\n', "\\n")
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
                // Ninja commands must be single-line. Encode newlines and
                // reconstruct the original script with `printf %b` piped into
                // a fresh shell to preserve expected expansions.
                let escaped = escape_script(script);
                writeln!(
                    f,
                    "  command = /bin/sh -e -c \"printf %b '{escaped}' | /bin/sh -e\""
                )?;
            }
            Recipe::Rule { .. } => unreachable!("rules do not reference other rules"),
        }
        write_kv!(f, "description", &self.action.description);
        write_kv!(f, "depfile", &self.action.depfile);
        write_kv!(f, "deps", &self.action.deps_format);
        write_kv!(f, "pool", &self.action.pool);
        write_flag!(f, "restat", self.action.restat);
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
        write_flag!(f, "restat", self.edge.always && !self.action_restat);
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
