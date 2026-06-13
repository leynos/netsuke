//! Private support functions for manifest-to-IR lowering.
//!
//! These helpers are kept separate from the main [`super`] module so the
//! `BuildGraph` implementation remains focused on orchestration while Kani can
//! still verify the production duplicate-output and rule-resolution paths.

use std::sync::Arc;

use camino::Utf8PathBuf;

use crate::ast::{Recipe, Rule, StringOrList};
use crate::hasher::ActionHasher;
use crate::localization::{self, keys};

use super::super::{
    cmd_interpolate::interpolate_command,
    graph::{Action, BuildEdge, IrGenError, IrHashMap},
};

#[derive(Clone, Copy)]
pub(super) struct ActionBindings<'a> {
    pub(super) inputs: &'a [Utf8PathBuf],
    pub(super) outputs: &'a [Utf8PathBuf],
}

pub(super) fn register_action(
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
pub(super) fn duplicate_output_error(
    outputs: &[Utf8PathBuf],
    targets: &IrHashMap<Utf8PathBuf, BuildEdge>,
) -> Option<IrGenError> {
    find_duplicates(outputs, targets).map(duplicate_output_error_from_paths)
}

/// Register one edge under each explicit output, moving the final edge.
pub(super) fn insert_edge_for_outputs(
    targets: &mut IrHashMap<Utf8PathBuf, BuildEdge>,
    edge: BuildEdge,
) {
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
        StringOrList::List(v) => {
            let mut mapped = Vec::new();
            let mut index = 0;
            while index < v.len() {
                if let Some(value) = v.get(index) {
                    mapped.push(f(value));
                }
                index += 1;
            }
            mapped
        }
    }
}

pub(super) fn to_paths(sol: &StringOrList) -> Vec<Utf8PathBuf> {
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
pub(super) fn resolve_rule(
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
                sort_strings(&mut rules);
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

fn sort_strings(values: &mut [String]) {
    let mut index = 1;
    while index < values.len() {
        let mut sorted_index = index;
        while should_swap_strings(values, sorted_index) {
            values.swap(sorted_index, sorted_index - 1);
            sorted_index -= 1;
        }
        index += 1;
    }
}

fn should_swap_strings(values: &[String], sorted_index: usize) -> bool {
    if sorted_index == 0 {
        return false;
    }
    let Some(current) = values.get(sorted_index) else {
        return false;
    };
    let Some(previous) = values.get(sorted_index - 1) else {
        return false;
    };
    string_cmp(current, previous) == std::cmp::Ordering::Less
}

#[cfg(not(kani))]
fn string_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    left.cmp(right)
}

#[cfg(kani)]
fn string_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    let left = left.as_bytes();
    let right = right.as_bytes();
    match (left.first(), right.first()) {
        (Some(left), Some(right)) => left.cmp(right),
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

/// Find output paths that would collide with existing or sibling outputs.
pub(super) fn find_duplicates(
    outputs: &[Utf8PathBuf],
    targets: &IrHashMap<Utf8PathBuf, BuildEdge>,
) -> Option<Vec<Utf8PathBuf>> {
    let mut seen: Vec<&Utf8PathBuf> = Vec::new();
    let mut dups = Vec::new();
    let mut index = 0;
    while index < outputs.len() {
        if let Some(output) = outputs.get(index) {
            if targets.contains_key(output) || has_seen_output(seen.as_slice(), output) {
                dups.push(output.clone());
            } else {
                seen.push(output);
            }
        }
        index += 1;
    }
    if dups.is_empty() {
        None
    } else {
        if dups.len() > 1 {
            sort_paths(&mut dups);
        }
        Some(dups)
    }
}

#[cfg(not(kani))]
fn sort_paths(paths: &mut [Utf8PathBuf]) {
    let mut index = 1;
    while index < paths.len() {
        let mut sorted_index = index;
        while should_swap_paths(paths, sorted_index) {
            paths.swap(sorted_index, sorted_index - 1);
            sorted_index -= 1;
        }
        index += 1;
    }
}

#[cfg(not(kani))]
fn should_swap_paths(paths: &[Utf8PathBuf], sorted_index: usize) -> bool {
    if sorted_index == 0 {
        return false;
    }
    let Some(current) = paths.get(sorted_index) else {
        return false;
    };
    let Some(previous) = paths.get(sorted_index - 1) else {
        return false;
    };
    path_cmp(current, previous) == std::cmp::Ordering::Less
}

#[cfg(kani)]
fn sort_paths(_paths: &mut [Utf8PathBuf]) {}

fn has_seen_output(seen: &[&Utf8PathBuf], output: &Utf8PathBuf) -> bool {
    let mut index = 0;
    while index < seen.len() {
        if let Some(candidate) = seen.get(index)
            && path_eq(candidate, output)
        {
            return true;
        }
        index += 1;
    }
    false
}

#[cfg(not(kani))]
fn path_eq(left: &Utf8PathBuf, right: &Utf8PathBuf) -> bool {
    left.as_str() == right.as_str()
}

#[cfg(kani)]
fn path_eq(left: &Utf8PathBuf, right: &Utf8PathBuf) -> bool {
    let left = left.as_str().as_bytes();
    let right = right.as_str().as_bytes();
    left.len() == 1 && right.len() == 1 && left[0] == right[0]
}

#[cfg(not(kani))]
fn path_cmp(left: &Utf8PathBuf, right: &Utf8PathBuf) -> std::cmp::Ordering {
    left.as_str().cmp(right.as_str())
}

pub(super) fn get_target_display_name(paths: &[Utf8PathBuf]) -> String {
    paths
        .first()
        .map(|p: &Utf8PathBuf| p.to_string())
        .unwrap_or_default()
}
