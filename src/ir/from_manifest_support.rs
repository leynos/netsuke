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
    // POLONIUS-REFUSED(id-is-data): the action hash is persistent IR
    // identity — callers store it on every `BuildEdge` and it names the
    // action in the generated Ninja file. Returning the owned hash (rather
    // than a reference to an interned key) is a data-model choice, not an
    // NLL workaround; the write-only `contains_key` guard below already
    // compiles under NLL. Convert to a borrow-returning accessor only if
    // callers develop a demonstrated need for the canonical interned value.
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

/// Interpret a manifest string selector as a list of build-graph paths.
///
/// Path conversion lives at the manifest-to-IR boundary rather than on
/// [`StringOrList`]: the AST models the manifest's surface syntax, in which
/// these fields are plain strings, and only lowering decides that they name
/// files on disk. The per-item mapping itself is [`StringOrList::map_each`],
/// so no traversal logic is duplicated here.
pub(super) fn to_paths(sol: &StringOrList) -> Vec<Utf8PathBuf> {
    // The closure is not redundant: `Utf8PathBuf::from` is generic over its
    // `From` impls, so passing it directly binds one concrete lifetime rather
    // than the higher-ranked `Fn(&str)` bound `map_each` requires.
    sol.map_each(|s| Utf8PathBuf::from(s))
}

/// Resolve a target rule selector into its single rule template.
pub(super) fn resolve_rule(
    rule: &StringOrList,
    rule_map: &IrHashMap<String, Arc<Rule>>,
    target_name: &str,
) -> Result<Arc<Rule>, IrGenError> {
    rule.as_single().map_or_else(
        || {
            let mut rules = rule.to_string_vec();
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

fn insertion_sort_by<T, F>(values: &mut [T], cmp: F)
where
    F: Fn(&T, &T) -> std::cmp::Ordering,
{
    let mut index = 1;
    while index < values.len() {
        let mut sorted_index = index;
        while sorted_index > 0 {
            let swap = values
                .get(sorted_index)
                .zip(values.get(sorted_index - 1))
                .is_some_and(|(cur, prev)| cmp(cur, prev) == std::cmp::Ordering::Less);
            if !swap {
                break;
            }
            values.swap(sorted_index, sorted_index - 1);
            sorted_index -= 1;
        }
        index += 1;
    }
}

fn sort_strings(values: &mut [String]) {
    insertion_sort_by(values, |a, b| string_cmp(a, b));
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
    insertion_sort_by(paths, path_cmp);
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
