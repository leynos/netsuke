//! Manifest-to-IR conversion helpers.

use std::collections::HashMap;
use std::sync::Arc;

use camino::Utf8PathBuf;

use crate::ast::{NetsukeManifest, Recipe, Rule, StringOrList};
use crate::hasher::ActionHasher;

use super::{
    cmd_interpolate::interpolate_command,
    cycle::{self, CycleDetectionReport},
    graph::{Action, BuildEdge, BuildGraph, IrGenError},
};

impl BuildGraph {
    /// Transform a manifest into a [`BuildGraph`].
    ///
    /// # Errors
    ///
    /// Returns [`IrGenError`] when a referenced rule is missing, multiple rules
    /// are specified for a single target, or no rule is provided.
    pub fn from_manifest(manifest: &NetsukeManifest) -> Result<Self, IrGenError> {
        let mut graph = Self::default();
        let mut rule_map: HashMap<String, Arc<Rule>> = HashMap::new();

        Self::process_rules(manifest, &mut rule_map);
        Self::process_targets(manifest, &mut graph.actions, &mut graph.targets, &rule_map)?;
        Self::process_defaults(manifest, &mut graph.default_targets);

        graph.detect_cycles()?;

        Ok(graph)
    }

    /// Collect rule templates without deduplicating them.
    ///
    /// Rules are stored verbatim and expanded later when targets reference
    /// them. This allows each target's input and output paths to be embedded in
    /// the resulting command, meaning identical rule definitions may yield
    /// distinct actions once interpolated. Should the manifest schema ever
    /// permit targets to override recipe fields such as `command` or
    /// `description`, those target-level values take precedence over the rule's
    /// defaults.
    fn process_rules(manifest: &NetsukeManifest, rule_map: &mut HashMap<String, Arc<Rule>>) {
        for rule in &manifest.rules {
            rule_map.insert(rule.name.clone(), Arc::new(rule.clone()));
        }
    }

    fn process_targets(
        manifest: &NetsukeManifest,
        actions: &mut HashMap<String, Action>,
        targets: &mut HashMap<Utf8PathBuf, BuildEdge>,
        rule_map: &HashMap<String, Arc<Rule>>,
    ) -> Result<(), IrGenError> {
        for target in manifest.actions.iter().chain(&manifest.targets) {
            let outputs = to_paths(&target.name);
            let inputs = to_paths(&target.sources);
            let target_name = get_target_display_name(&outputs);
            let action_id = match &target.recipe {
                Recipe::Rule { rule } => {
                    let tmpl = resolve_rule(rule, rule_map, &target_name)?;
                    // Future schema versions may allow targets to override
                    // recipe or description fields. If so, those values will
                    // take precedence over the rule template.
                    register_action(
                        actions,
                        tmpl.recipe.clone(),
                        tmpl.description.clone(),
                        &inputs,
                        &outputs,
                    )?
                }
                Recipe::Command { .. } | Recipe::Script { .. } => {
                    register_action(actions, target.recipe.clone(), None, &inputs, &outputs)?
                }
            };

            let edge = BuildEdge {
                action_id,
                inputs: inputs.clone(),
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

    fn process_defaults(manifest: &NetsukeManifest, defaults: &mut Vec<Utf8PathBuf>) {
        defaults.extend(manifest.defaults.iter().map(Utf8PathBuf::from));
    }

    fn detect_cycles(&self) -> Result<(), IrGenError> {
        let CycleDetectionReport {
            cycle,
            missing_dependencies,
        } = cycle::analyse(&self.targets);
        if let Some(cycle) = cycle {
            return Err(IrGenError::CircularDependency {
                cycle,
                missing_dependencies,
            });
        }
        Ok(())
    }
}

fn register_action(
    actions: &mut HashMap<String, Action>,
    recipe: Recipe,
    description: Option<String>,
    inputs: &[Utf8PathBuf],
    outputs: &[Utf8PathBuf],
) -> Result<String, IrGenError> {
    let recipe = match recipe {
        Recipe::Command { command } => {
            let interpolated = interpolate_command(&command, inputs, outputs)?;
            Recipe::Command {
                command: interpolated,
            }
        }
        other => other,
    };
    let action = Action {
        recipe,
        description,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let hash = ActionHasher::hash(&action).map_err(IrGenError::ActionSerialisation)?;
    actions.entry(hash.clone()).or_insert(action);
    Ok(hash)
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

fn to_paths(sol: &StringOrList) -> Vec<Utf8PathBuf> {
    map_string_or_list(sol, |s| Utf8PathBuf::from(s))
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
    rule_map: &HashMap<String, Arc<Rule>>,
    target_name: &str,
) -> Result<Arc<Rule>, IrGenError> {
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
    outputs: &[Utf8PathBuf],
    targets: &HashMap<Utf8PathBuf, BuildEdge>,
) -> Option<Vec<String>> {
    let mut dups: Vec<String> = outputs
        .iter()
        .filter(|o| targets.contains_key(*o))
        .map(|p| p.as_str().to_owned())
        .collect();
    if dups.is_empty() {
        None
    } else {
        dups.sort();
        Some(dups)
    }
}

fn get_target_display_name(paths: &[Utf8PathBuf]) -> String {
    paths.first().map(ToString::to_string).unwrap_or_default()
}
