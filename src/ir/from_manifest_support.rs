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
    cmd_interpolate::{CommandBindings, interpolate_command_with_bindings},
    graph::{Action, BuildEdge, IrGenError, IrHashMap},
};

#[path = "sort_utils.rs"]
mod sort_utils;

/// The `$in`/`$out` substitution views for one action under construction.
#[derive(Clone, Copy)]
pub(super) struct ActionBindings<'a> {
    /// Paths the target's recipe consumes via `$in`.
    pub(super) inputs: &'a [Utf8PathBuf],
    /// Paths the target's recipe produces via `$out`.
    pub(super) outputs: &'a [Utf8PathBuf],
}

/// Register one action under its content hash, deduplicating identical ones.
///
/// Command recipes interpolate their `$in`/`$out` bindings; equivalent
/// expanded commands share one action hash.
///
/// # Errors
///
/// Returns [`IrGenError::ActionSerialisation`] when the action cannot be
/// hashed or serialized.
pub(super) fn register_action(
    actions: &mut IrHashMap<String, Action>,
    recipe: Recipe,
    description: Option<&str>,
    bindings: ActionBindings<'_>,
) -> Result<String, IrGenError> {
    let resolved_recipe = match recipe {
        Recipe::Command { command } => {
            let command_bindings = CommandBindings::new(bindings.inputs, bindings.outputs);
            let interpolated = match command {
                StringOrList::String(cmd) => StringOrList::String(
                    interpolate_command_with_bindings(&cmd, &command_bindings)?,
                ),
                StringOrList::List(items) => {
                    let mut rendered = Vec::with_capacity(items.len());
                    for item in items {
                        rendered.push(interpolate_command_with_bindings(&item, &command_bindings)?);
                    }
                    StringOrList::List(rendered)
                }
                // An empty command list cannot deserialize (the manifest
                // parser rejects it), so nothing needs interpolating here.
                StringOrList::Empty => StringOrList::Empty,
            };
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

/// Build the duplicate-output error for the reported colliding paths.
fn duplicate_output_error_from_paths(dups: Vec<Utf8PathBuf>) -> IrGenError {
    let message = duplicate_outputs_message(&dups);
    IrGenError::DuplicateOutput {
        message,
        outputs: dups.into_iter().map(|p| p.as_str().to_owned()).collect(),
    }
}

/// Localise the duplicate-output message with the offending output paths.
fn duplicate_outputs_message(dups: &[Utf8PathBuf]) -> localization::LocalizedMessage {
    add_debug_arg(
        localization::message(keys::IR_DUPLICATE_OUTPUTS),
        "outputs",
        dups,
    )
}

/// Attach one named argument to a localized message.
#[cfg(not(kani))]
fn add_arg<T: ToString + ?Sized>(
    message: localization::LocalizedMessage,
    key: &'static str,
    value: &T,
) -> localization::LocalizedMessage {
    message.with_arg(key, value.to_string())
}

/// Attach one named argument to a localized message; a no-op in the Kani
/// build.
#[cfg(kani)]
fn add_arg<T: ?Sized>(
    message: localization::LocalizedMessage,
    _key: &'static str,
    _value: &T,
) -> localization::LocalizedMessage {
    message
}

/// Attach one debug-formatted argument to a localized message.
#[cfg(not(kani))]
fn add_debug_arg(
    message: localization::LocalizedMessage,
    key: &'static str,
    value: impl std::fmt::Debug,
) -> localization::LocalizedMessage {
    let rendered = format!("{value:?}");
    add_arg(message, key, &rendered)
}

/// Attach one debug-formatted argument to a localized message; a no-op in
/// the Kani build.
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
                sort_utils::sort_strings(&mut rules);
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

/// Build the empty-rule error for a target that selected no rule.
fn empty_rule_error(target_name: &str) -> IrGenError {
    IrGenError::EmptyRule {
        target_name: target_name.to_owned(),
        message: empty_rule_message(target_name),
    }
}

/// Build the multiple-rules error for a target selecting several rules.
fn multiple_rules_error(target_name: &str, rules: Vec<String>) -> IrGenError {
    IrGenError::MultipleRules {
        target_name: target_name.to_owned(),
        message: multiple_rules_message(target_name, &rules),
        rules,
    }
}

/// Build the rule-not-found error for an unknown single-rule selector.
fn rule_not_found_error(target_name: &str, rule_name: &str) -> IrGenError {
    IrGenError::RuleNotFound {
        target_name: target_name.to_owned(),
        rule_name: rule_name.to_owned(),
        message: rule_not_found_message(target_name, rule_name),
    }
}

/// Localise the empty-rule message with the target name.
fn empty_rule_message(target_name: &str) -> localization::LocalizedMessage {
    add_arg(
        localization::message(keys::IR_EMPTY_RULE),
        "target",
        target_name,
    )
}

/// Localise the multiple-rules message with the target and rule names.
fn multiple_rules_message(target_name: &str, rules: &[String]) -> localization::LocalizedMessage {
    let message = localization::message(keys::IR_MULTIPLE_RULES);
    let with_target = add_arg(message, "target", target_name);
    add_debug_arg(with_target, "rules", rules)
}

/// Localise the rule-not-found message with the target and rule names.
fn rule_not_found_message(target_name: &str, rule_name: &str) -> localization::LocalizedMessage {
    let message = localization::message(keys::IR_RULE_NOT_FOUND);
    let with_target = add_arg(message, "target", target_name);
    add_arg(with_target, "rule", rule_name)
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
            if targets.contains_key(output) || sort_utils::has_seen_output(seen.as_slice(), output)
            {
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
            sort_utils::sort_paths(&mut dups);
        }
        Some(dups)
    }
}

/// Return the first output path as the target's display name, or an empty
/// string when the target declares no explicit outputs.
pub(super) fn get_target_display_name(paths: &[Utf8PathBuf]) -> String {
    paths
        .first()
        .map(|p: &Utf8PathBuf| p.to_string())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "from_manifest_support_tests.rs"]
mod tests;
