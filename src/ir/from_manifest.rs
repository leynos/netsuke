//! Manifest-to-IR conversion helpers.
//!
//! Implements [`BuildGraph::from_manifest`], which lowers a parsed
//! [`crate::ast::NetsukeManifest`] into a [`BuildGraph`].  Delegates
//! template rendering to [`crate::manifest::render`], command interpolation
//! to [`super::cmd_interpolate`], and cycle/missing-dependency detection to
//! [`super::cycle`].

use std::sync::Arc;

use camino::Utf8PathBuf;

use crate::ast::{NetsukeManifest, Recipe, Rule, StringOrList};
use crate::hasher::ActionHasher;
use crate::localization::{self, keys};

use super::{
    cmd_interpolate::interpolate_command,
    cycle::{self, CycleDetectionReport},
    graph::{Action, BuildEdge, BuildGraph, IrGenError, IrHashMap},
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
        let mut rule_map = IrHashMap::<String, Arc<Rule>>::default();

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
    fn process_rules(manifest: &NetsukeManifest, rule_map: &mut IrHashMap<String, Arc<Rule>>) {
        for rule in &manifest.rules {
            rule_map.insert(rule.name.clone(), Arc::new(rule.clone()));
        }
    }

    fn process_targets(
        manifest: &NetsukeManifest,
        actions: &mut IrHashMap<String, Action>,
        targets: &mut IrHashMap<Utf8PathBuf, BuildEdge>,
        rule_map: &IrHashMap<String, Arc<Rule>>,
    ) -> Result<(), IrGenError> {
        for target in manifest.actions.iter().chain(&manifest.targets) {
            let outputs = to_paths(&target.name);
            let inputs = to_paths(&target.sources);
            let implicit_deps = to_paths(&target.deps);
            tracing::debug!(
                target = ?target.name,
                implicit_deps_count = implicit_deps.len(),
                "populating implicit dependencies for target",
            );
            if let Some(error) = duplicate_output_error(&outputs, targets) {
                return Err(error);
            }

            let action_id = match &target.recipe {
                Recipe::Rule { rule } => {
                    let target_name = get_target_display_name(&outputs);
                    let tmpl = resolve_rule(rule, rule_map, &target_name)?;
                    // Future schema versions may allow targets to override
                    // recipe or description fields. If so, those values will
                    // take precedence over the rule template.
                    register_action(
                        actions,
                        tmpl.recipe.clone(),
                        tmpl.description.as_deref(),
                        ActionBindings {
                            inputs: &inputs,
                            outputs: &outputs,
                        },
                    )?
                }
                Recipe::Command { .. } | Recipe::Script { .. } => register_action(
                    actions,
                    target.recipe.clone(),
                    None,
                    ActionBindings {
                        inputs: &inputs,
                        outputs: &outputs,
                    },
                )?,
            };

            let edge = BuildEdge {
                action_id,
                inputs,
                implicit_deps,
                explicit_outputs: outputs,
                implicit_outputs: Vec::new(),
                order_only_deps: to_paths(&target.order_only_deps),
                phony: target.phony,
                always: target.always,
            };

            insert_edge_for_outputs(targets, edge);
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

        for (dependent, missing) in &missing_dependencies {
            tracing::info!(
                dependent = %dependent,
                missing = %missing,
                "unresolved dependency: not a build target; assuming it is an external file",
            );
        }

        if let Some(detected_cycle) = cycle {
            let message = localization::message(keys::IR_CIRCULAR_DEPENDENCY)
                .with_arg("cycle", format!("{detected_cycle:?}"));
            return Err(IrGenError::CircularDependency {
                cycle: detected_cycle,
                missing_dependencies,
                message,
            });
        }

        tracing::info!(
            count = missing_dependencies.len(),
            "cycle detection complete; unresolved dependencies treated as external files",
        );
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct ActionBindings<'a> {
    inputs: &'a [Utf8PathBuf],
    outputs: &'a [Utf8PathBuf],
}

fn register_action(
    actions: &mut IrHashMap<String, Action>,
    recipe: Recipe,
    description: Option<&str>,
    bindings: ActionBindings<'_>,
) -> Result<String, IrGenError> {
    let resolved_recipe = match recipe {
        Recipe::Command { command } => {
            let interpolated = interpolate_command(&command, bindings.inputs, bindings.outputs)?;
            Recipe::Command {
                command: interpolated,
            }
        }
        other => other,
    };
    let action = Action {
        recipe: resolved_recipe,
        description: description.map(ToOwned::to_owned),
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let hash = ActionHasher::hash(&action).map_err(|err| IrGenError::ActionSerialisation {
        message: localization::message(keys::IR_ACTION_SERIALISATION)
            .with_arg("details", err.to_string()),
        source: err,
    })?;
    if !actions.contains_key(hash.as_str()) {
        actions.insert(hash.clone(), action);
    }
    Ok(hash)
}

/// Report duplicate outputs already known or repeated within one target.
fn duplicate_output_error(
    outputs: &[Utf8PathBuf],
    targets: &IrHashMap<Utf8PathBuf, BuildEdge>,
) -> Option<IrGenError> {
    find_duplicates(outputs, targets).map(duplicate_output_error_from_paths)
}

/// Register one edge under each explicit output, moving the final edge.
fn insert_edge_for_outputs(targets: &mut IrHashMap<Utf8PathBuf, BuildEdge>, edge: BuildEdge) {
    if let Some((last_output, other_outputs)) = edge.explicit_outputs.split_last() {
        for output in other_outputs {
            targets.insert(output.clone(), edge.clone());
        }
        targets.insert(last_output.clone(), edge);
    }
}

fn duplicate_output_error_from_paths(dups: Vec<Utf8PathBuf>) -> IrGenError {
    let message = duplicate_outputs_message(&dups);
    IrGenError::DuplicateOutput {
        message,
        outputs: dups.into_iter().map(|p| p.as_str().to_owned()).collect(),
    }
}

fn duplicate_outputs_message(dups: &[Utf8PathBuf]) -> localization::LocalizedMessage {
    add_debug_arg(
        localization::message(keys::IR_DUPLICATE_OUTPUTS),
        "outputs",
        dups,
    )
}

#[cfg(not(kani))]
fn add_arg<T: ToString + ?Sized>(
    message: localization::LocalizedMessage,
    key: &'static str,
    value: &T,
) -> localization::LocalizedMessage {
    message.with_arg(key, value.to_string())
}

#[cfg(kani)]
fn add_arg<T: ?Sized>(
    message: localization::LocalizedMessage,
    _key: &'static str,
    _value: &T,
) -> localization::LocalizedMessage {
    message
}

#[cfg(not(kani))]
fn add_debug_arg(
    message: localization::LocalizedMessage,
    key: &'static str,
    value: impl std::fmt::Debug,
) -> localization::LocalizedMessage {
    let rendered = format!("{value:?}");
    add_arg(message, key, &rendered)
}

#[cfg(kani)]
fn add_debug_arg<T: ?Sized>(
    message: localization::LocalizedMessage,
    _key: &'static str,
    _value: &T,
) -> localization::LocalizedMessage {
    message
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
    map_string_or_list(sol, str::to_owned)
}

fn extract_single(sol: &StringOrList) -> Option<&str> {
    match sol {
        StringOrList::String(s) => Some(s),
        StringOrList::List(v) if v.len() == 1 => v.first().map(String::as_str),
        _ => None,
    }
}

/// Resolve a target rule selector into its single rule template.
fn resolve_rule(
    rule: &StringOrList,
    rule_map: &IrHashMap<String, Arc<Rule>>,
    target_name: &str,
) -> Result<Arc<Rule>, IrGenError> {
    extract_single(rule).map_or_else(
        || {
            let mut rules = to_string_vec(rule);
            if rules.is_empty() {
                Err(empty_rule_error(target_name))
            } else {
                rules.sort();
                Err(multiple_rules_error(target_name, rules))
            }
        },
        |name| {
            rule_map
                .get(name)
                .cloned()
                .ok_or_else(|| rule_not_found_error(target_name, name))
        },
    )
}

fn empty_rule_error(target_name: &str) -> IrGenError {
    IrGenError::EmptyRule {
        target_name: target_name.to_owned(),
        message: empty_rule_message(target_name),
    }
}

fn multiple_rules_error(target_name: &str, rules: Vec<String>) -> IrGenError {
    IrGenError::MultipleRules {
        target_name: target_name.to_owned(),
        message: multiple_rules_message(target_name, &rules),
        rules,
    }
}

fn rule_not_found_error(target_name: &str, rule_name: &str) -> IrGenError {
    IrGenError::RuleNotFound {
        target_name: target_name.to_owned(),
        rule_name: rule_name.to_owned(),
        message: rule_not_found_message(target_name, rule_name),
    }
}

fn empty_rule_message(target_name: &str) -> localization::LocalizedMessage {
    add_arg(
        localization::message(keys::IR_EMPTY_RULE),
        "target",
        target_name,
    )
}

fn multiple_rules_message(target_name: &str, rules: &[String]) -> localization::LocalizedMessage {
    let message = localization::message(keys::IR_MULTIPLE_RULES);
    let with_target = add_arg(message, "target", target_name);
    add_debug_arg(with_target, "rules", rules)
}

fn rule_not_found_message(target_name: &str, rule_name: &str) -> localization::LocalizedMessage {
    let message = localization::message(keys::IR_RULE_NOT_FOUND);
    let with_target = add_arg(message, "target", target_name);
    add_arg(with_target, "rule", rule_name)
}

/// Find output paths that would collide with existing or sibling outputs.
fn find_duplicates(
    outputs: &[Utf8PathBuf],
    targets: &IrHashMap<Utf8PathBuf, BuildEdge>,
) -> Option<Vec<Utf8PathBuf>> {
    let mut seen: Vec<&Utf8PathBuf> = Vec::new();
    let mut dups = Vec::new();
    for output in outputs {
        if targets.contains_key(output)
            || seen.iter().any(|seen_output| path_eq(seen_output, output))
        {
            dups.push(output.clone());
        } else {
            seen.push(output);
        }
    }
    if dups.is_empty() {
        None
    } else {
        dups.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        Some(dups)
    }
}

fn path_eq(left: &Utf8PathBuf, right: &Utf8PathBuf) -> bool {
    left.as_str() == right.as_str()
}

fn get_target_display_name(paths: &[Utf8PathBuf]) -> String {
    paths
        .first()
        .map(|p: &Utf8PathBuf| p.to_string())
        .unwrap_or_default()
}

#[cfg(kani)]
#[path = "from_manifest_verification.rs"]
mod verification;
