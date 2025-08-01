//! Intermediate Representation structures.
//!
//! This module defines the backend-agnostic build graph used by Netsuke after
//! validation. The IR mirrors the conceptual model of Ninja without embedding
//! any Ninja-specific syntax.
//!
//! # Examples
//!
//! ```
//! use netsuke::ir::{Action, BuildGraph, BuildEdge};
//! use netsuke::ast::Recipe;
//! use std::path::PathBuf;
//!
//! let action = Action {
//!     recipe: Recipe::Command { command: "echo hi".into() },
//!     description: None,
//!     depfile: None,
//!     deps_format: None,
//!     pool: None,
//!     restat: false,
//! };
//! let mut graph = BuildGraph::default();
//! graph.actions.insert("a".into(), action);
//! graph.default_targets.push(PathBuf::from("hello"));
//! ```
//
use std::collections::HashMap;
use std::path::PathBuf;

/// The complete, static build graph.
#[derive(Debug, Default, Clone)]
pub struct BuildGraph {
    /// All unique actions in the build keyed by a stable hash.
    pub actions: HashMap<String, Action>,
    /// All target files to be built keyed by output path.
    pub targets: HashMap<PathBuf, BuildEdge>,
    /// Targets built when no explicit target is requested.
    pub default_targets: Vec<PathBuf>,
}

/// A reusable command analogous to a Ninja rule.
#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub recipe: Recipe,
    pub description: Option<String>,
    pub depfile: Option<String>,
    pub deps_format: Option<String>,
    pub pool: Option<String>,
    pub restat: bool,
}

/// A single build statement connecting inputs to outputs.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildEdge {
    /// Identifier of the [`Action`] used for this edge.
    pub action_id: String,
    /// Explicit inputs that trigger a rebuild when changed.
    pub inputs: Vec<PathBuf>,
    /// Outputs explicitly generated by the command.
    pub explicit_outputs: Vec<PathBuf>,
    /// Outputs implicitly generated by the command (Ninja `|`).
    pub implicit_outputs: Vec<PathBuf>,
    /// Order-only dependencies that do not trigger rebuilds (Ninja `||`).
    pub order_only_deps: Vec<PathBuf>,
    /// Always run the command even if the output exists.
    pub phony: bool,
    /// Run the command on every invocation regardless of timestamps.
    pub always: bool,
}

use crate::ast::{NetsukeManifest, Recipe, StringOrList};
use thiserror::Error;

use crate::hasher::ActionHasher;

/// Errors produced during IR generation.
#[derive(Debug, Error)]
pub enum IrGenError {
    #[error("rule '{rule_name}' referenced by target '{target_name}' was not found")]
    RuleNotFound {
        target_name: String,
        rule_name: String,
    },

    #[error("multiple rules for target '{target_name}': {rules:?}")]
    MultipleRules {
        target_name: String,
        rules: Vec<String>,
    },

    #[error("No rules specified for target {target_name}")]
    EmptyRule { target_name: String },

    #[error("duplicate target outputs: {outputs:?}")]
    DuplicateOutput { outputs: Vec<String> },

    #[error("circular dependency detected: {cycle:?}")]
    CircularDependency { cycle: Vec<PathBuf> },
}

impl BuildGraph {
    /// Transform a manifest into a [`BuildGraph`].
    ///
    /// # Errors
    ///
    /// Returns [`IrGenError`] when a referenced rule is missing, multiple rules
    /// are specified for a single target, or no rule is provided.
    pub fn from_manifest(manifest: &NetsukeManifest) -> Result<Self, IrGenError> {
        let mut graph = Self::default();
        let mut rule_map = HashMap::new();

        Self::process_rules(manifest, &mut graph.actions, &mut rule_map);
        Self::process_targets(manifest, &mut graph.actions, &mut graph.targets, &rule_map)?;
        Self::process_defaults(manifest, &mut graph.default_targets);

        graph.detect_cycles()?;

        Ok(graph)
    }

    fn process_rules(
        manifest: &NetsukeManifest,
        actions: &mut HashMap<String, Action>,
        rule_map: &mut HashMap<String, String>,
    ) {
        for rule in &manifest.rules {
            let hash = register_action(actions, rule.recipe.clone(), rule.description.clone());
            rule_map.insert(rule.name.clone(), hash);
        }
    }

    fn process_targets(
        manifest: &NetsukeManifest,
        actions: &mut HashMap<String, Action>,
        targets: &mut HashMap<PathBuf, BuildEdge>,
        rule_map: &HashMap<String, String>,
    ) -> Result<(), IrGenError> {
        for target in manifest.actions.iter().chain(&manifest.targets) {
            let outputs = to_paths(&target.name);
            let target_name = get_target_display_name(&outputs);
            let action_id = match &target.recipe {
                Recipe::Rule { rule } => resolve_rule(rule, rule_map, &target_name)?,
                Recipe::Command { .. } | Recipe::Script { .. } => {
                    register_action(actions, target.recipe.clone(), None)
                }
            };

            let edge = BuildEdge {
                action_id,
                inputs: to_paths(&target.sources),
                explicit_outputs: outputs.clone(),
                implicit_outputs: Vec::new(),
                order_only_deps: to_paths(&target.order_only_deps),
                phony: target.phony,
                always: target.always,
            };

            if let Some(dups) = find_duplicates(&outputs, targets) {
                return Err(IrGenError::DuplicateOutput { outputs: dups });
            }
            for out in outputs {
                targets.insert(out, edge.clone());
            }
        }
        Ok(())
    }

    fn process_defaults(manifest: &NetsukeManifest, defaults: &mut Vec<PathBuf>) {
        for name in &manifest.defaults {
            defaults.push(PathBuf::from(name));
        }
    }

    fn detect_cycles(&self) -> Result<(), IrGenError> {
        if let Some(cycle) = find_cycle(&self.targets) {
            return Err(IrGenError::CircularDependency { cycle });
        }
        Ok(())
    }
}

fn register_action(
    actions: &mut HashMap<String, Action>,
    recipe: Recipe,
    description: Option<String>,
) -> String {
    let action = Action {
        recipe,
        description,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let hash = ActionHasher::hash(&action);
    actions.entry(hash.clone()).or_insert(action);
    hash
}

fn map_string_or_list<T, F>(sol: &StringOrList, f: F) -> Vec<T>
where
    F: Fn(&str) -> T,
{
    match sol {
        StringOrList::Empty => Vec::new(),
        StringOrList::String(s) => vec![f(s)],
        StringOrList::List(v) => v.iter().map(|s| f(s)).collect(),
    }
}

fn to_paths(sol: &StringOrList) -> Vec<PathBuf> {
    map_string_or_list(sol, |s| PathBuf::from(s))
}

fn to_string_vec(sol: &StringOrList) -> Vec<String> {
    map_string_or_list(sol, str::to_string)
}

fn extract_single(sol: &StringOrList) -> Option<&str> {
    match sol {
        StringOrList::String(s) => Some(s),
        StringOrList::List(v) if v.len() == 1 => v.first().map(String::as_str),
        _ => None,
    }
}

fn resolve_rule(
    rule: &StringOrList,
    rule_map: &HashMap<String, String>,
    target_name: &str,
) -> Result<String, IrGenError> {
    extract_single(rule).map_or_else(
        || {
            let mut rules = to_string_vec(rule);
            if rules.is_empty() {
                Err(IrGenError::EmptyRule {
                    target_name: target_name.to_string(),
                })
            } else {
                rules.sort();
                Err(IrGenError::MultipleRules {
                    target_name: target_name.to_string(),
                    rules,
                })
            }
        },
        |name| {
            rule_map
                .get(name)
                .cloned()
                .ok_or_else(|| IrGenError::RuleNotFound {
                    target_name: target_name.to_string(),
                    rule_name: name.to_string(),
                })
        },
    )
}

fn find_duplicates(
    outputs: &[PathBuf],
    targets: &HashMap<PathBuf, BuildEdge>,
) -> Option<Vec<String>> {
    let mut dups: Vec<_> = outputs
        .iter()
        .filter(|o| targets.contains_key(*o))
        .map(|o| o.display().to_string())
        .collect();
    if dups.is_empty() {
        None
    } else {
        dups.sort();
        Some(dups)
    }
}

fn get_target_display_name(paths: &[PathBuf]) -> String {
    paths
        .first()
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

#[derive(Clone, Copy)]
enum VisitState {
    Visiting,
    Visited,
}

fn should_visit_node<'a>(
    states: &'a mut HashMap<PathBuf, VisitState>,
    node: &'a PathBuf,
) -> Result<bool, &'a PathBuf> {
    match states.get(node) {
        Some(VisitState::Visited) => Ok(false),
        Some(VisitState::Visiting) => Err(node),
        None => {
            states.insert(node.clone(), VisitState::Visiting);
            Ok(true)
        }
    }
}

fn find_cycle(targets: &HashMap<PathBuf, BuildEdge>) -> Option<Vec<PathBuf>> {
    fn visit(
        targets: &HashMap<PathBuf, BuildEdge>,
        node: &PathBuf,
        stack: &mut Vec<PathBuf>,
        states: &mut HashMap<PathBuf, VisitState>,
    ) -> Option<Vec<PathBuf>> {
        match should_visit_node(states, node) {
            Ok(false) => return None,
            Err(path) => {
                if let Some(idx) = stack.iter().position(|n| n == path) {
                    let mut cycle = stack.get(idx..).expect("slice").to_vec();
                    cycle.push(path.clone());
                    return Some(canonicalize_cycle(cycle));
                }
                return Some(vec![path.clone(), path.clone()]);
            }
            Ok(true) => {}
        }

        stack.push(node.clone());

        if let Some(edge) = targets.get(node)
            && let Some(cycle) = visit_dependencies(targets, &edge.inputs, stack, states)
        {
            return Some(cycle);
        }

        stack.pop();
        states.insert(node.clone(), VisitState::Visited);
        None
    }

    fn visit_dependencies(
        targets: &HashMap<PathBuf, BuildEdge>,
        deps: &[PathBuf],
        stack: &mut Vec<PathBuf>,
        states: &mut HashMap<PathBuf, VisitState>,
    ) -> Option<Vec<PathBuf>> {
        for dep in deps {
            if targets.contains_key(dep)
                && let Some(cycle) = visit(targets, dep, stack, states)
            {
                return Some(cycle);
            }
        }
        None
    }

    let mut states = HashMap::new();
    let mut stack = Vec::new();

    for node in targets.keys() {
        if !states.contains_key(node)
            && let Some(cycle) = visit(targets, node, &mut stack, &mut states)
        {
            return Some(cycle);
        }
    }
    None
}

fn canonicalize_cycle(mut cycle: Vec<PathBuf>) -> Vec<PathBuf> {
    if cycle.len() < 2 {
        return cycle;
    }
    let len = cycle.len() - 1;
    let start = cycle
        .iter()
        .take(len)
        .enumerate()
        .min_by(|(_, a), (_, b)| a.cmp(b))
        .map_or(0, |(idx, _)| idx);
    cycle.rotate_left(start);
    if let (Some(first), Some(slot)) = (cycle.first().cloned(), cycle.get_mut(len)) {
        slot.clone_from(&first);
    }
    cycle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_cycle_identifies_cycle() {
        let mut targets = HashMap::new();
        let edge_a = BuildEdge {
            action_id: "id".into(),
            inputs: vec![PathBuf::from("b")],
            explicit_outputs: vec![PathBuf::from("a")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
        };
        let edge_b = BuildEdge {
            action_id: "id".into(),
            inputs: vec![PathBuf::from("a")],
            explicit_outputs: vec![PathBuf::from("b")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
        };
        targets.insert(PathBuf::from("a"), edge_a);
        targets.insert(PathBuf::from("b"), edge_b);

        let cycle = find_cycle(&targets).expect("cycle");
        let option_a = vec![PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("a")];
        let option_b = vec![PathBuf::from("b"), PathBuf::from("a"), PathBuf::from("b")];
        assert!(cycle == option_a || cycle == option_b);
    }
}
