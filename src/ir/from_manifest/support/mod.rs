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
use crate::recipe_shell::RecipeShell;

use self::command_resolution::{resolve_command, resolve_script};
#[cfg(kani)]
use super::super::graph::EdgeId;
use super::super::graph::{Action, BuildEdge, BuildGraph, IrGenError, IrHashMap};

mod command_resolution;
mod sort_utils;

/// The `{{ ins }}`/`{{ outs }}` substitution views for one action under construction.
#[derive(Clone, Copy)]
pub(super) struct ActionBindings<'a> {
    /// Paths the target's recipe consumes via `{{ ins }}`.
    pub(super) inputs: &'a [Utf8PathBuf],
    /// Paths the target's recipe produces via `{{ outs }}`.
    pub(super) outputs: &'a [Utf8PathBuf],
    /// Shell-specific lowering context for recipe paths in the legacy recipe.
    pub(super) shell: RecipeShell,
}

/// Register one action under its content hash, deduplicating identical ones.
///
/// Command recipes interpolate their `{{ ins }}`/`{{ outs }}` bindings; equivalent
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
    let action = Action {
        recipe: resolve_recipe(recipe, bindings)?,
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

/// Resolves action bindings in the recipe variant supplied by a manifest.
fn resolve_recipe(recipe: Recipe, bindings: ActionBindings<'_>) -> Result<Recipe, IrGenError> {
    match recipe {
        Recipe::Command {
            command, isolation, ..
        } => Ok(Recipe::Command {
            command: resolve_command(command, bindings, isolation)?,
            isolation,
        }),
        Recipe::Script { script } => Ok(Recipe::Script {
            script: resolve_script(&script, bindings)?,
        }),
        rule @ Recipe::Rule { .. } => Ok(rule),
    }
}

/// Report duplicate outputs already known or repeated within one target.
#[cfg(not(kani))]
pub(super) fn duplicate_output_error(
    outputs: &[Utf8PathBuf],
    graph: &BuildGraph,
) -> Option<IrGenError> {
    find_duplicate_graph_outputs(outputs, graph).map(duplicate_output_error_from_paths)
}

/// Report duplicate outputs through Kani's bounded output index.
#[cfg(kani)]
pub(super) fn duplicate_output_error(
    outputs: &[Utf8PathBuf],
    graph: &BuildGraph,
) -> Option<IrGenError> {
    find_duplicates(outputs, graph.output_index()).map(duplicate_output_error_from_paths)
}

/// Register one canonical edge under each output alias.
#[cfg(not(kani))]
pub(super) fn insert_edge_for_outputs(
    graph: &mut BuildGraph,
    edge: BuildEdge,
) -> Result<(), IrGenError> {
    graph.insert_edge(edge).map(|_| ())
}

/// Store one canonical edge in Kani's bounded graph model.
#[cfg(kani)]
pub(super) fn insert_edge_for_outputs(graph: &mut BuildGraph, edge: BuildEdge) {
    graph.insert_edge(edge);
}

/// Build the duplicate-output error for the reported colliding paths.
fn duplicate_output_error_from_paths(dups: Vec<Utf8PathBuf>) -> IrGenError {
    let message = duplicate_outputs_message(&dups);
    IrGenError::DuplicateOutput {
        message,
        outputs: dups.into_iter().map(|p| p.as_str().to_owned()).collect(),
    }
}

/// Localize the duplicate-output message with the offending output paths.
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
///
/// A selector names exactly one rule, as a scalar or a one-element list. The
/// fallback therefore rejects rather than searches, and it inspects the variant
/// directly: rejecting after cloning the whole selector would make a compact
/// invalid manifest cost the process a full copy and sort of every name.
pub(super) fn resolve_rule(
    rule: &StringOrList,
    rule_map: &IrHashMap<String, Arc<Rule>>,
    target_name: &str,
) -> Result<Arc<Rule>, IrGenError> {
    if let Some(name) = rule.as_single() {
        return rule_map
            .get(name)
            .cloned()
            .ok_or_else(|| rule_not_found_error(target_name, name));
    }
    match rule {
        StringOrList::List(names) if !names.is_empty() => {
            let rules = sort_utils::smallest_names(names, sort_utils::MAX_REPORTED_RULE_SELECTORS);
            let omitted = names.len() - rules.len();
            Err(multiple_rules_error(target_name, rules, omitted))
        }
        // `String` and one-element lists are answered by `as_single`, so only
        // the empty shapes reach this arm.
        _ => Err(empty_rule_error(target_name)),
    }
}

/// Build the empty-rule error for a target that selected no rule.
fn empty_rule_error(target_name: &str) -> IrGenError {
    IrGenError::EmptyRule {
        target_name: target_name.to_owned(),
        message: empty_rule_message(target_name),
    }
}

/// Build the multiple-rules error for a target selecting several rules.
///
/// `rules` is the bounded sorted sample and `omitted` counts the names the
/// sample leaves out, so the error carries both halves of the diagnostic.
fn multiple_rules_error(target_name: &str, rules: Vec<String>, omitted: usize) -> IrGenError {
    IrGenError::MultipleRules {
        target_name: target_name.to_owned(),
        message: multiple_rules_message(target_name, &rules, omitted),
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

/// Localize the empty-rule message with the target name.
fn empty_rule_message(target_name: &str) -> localization::LocalizedMessage {
    add_arg(
        localization::message(keys::IR_EMPTY_RULE),
        "target",
        target_name,
    )
}

/// Localize the multiple-rules message with the target and rule names.
///
/// A selector reported in full keeps the existing message exactly; only a
/// truncated sample switches to the message that states how many names it
/// leaves out. The count is attached with [`localization::LocalizedMessage::with_count`]
/// so Fluent can select its plural variant, rather than as pre-rendered text.
fn multiple_rules_message(
    target_name: &str,
    rules: &[String],
    omitted: usize,
) -> localization::LocalizedMessage {
    let key = if omitted == 0 {
        keys::IR_MULTIPLE_RULES
    } else {
        keys::IR_MULTIPLE_RULES_TRUNCATED
    };
    let message = localization::message(key);
    let with_target = add_arg(message, "target", target_name);
    let with_rules = add_debug_arg(with_target, "rules", rules);
    if omitted == 0 {
        with_rules
    } else {
        with_rules.with_count("omitted", omitted)
    }
}

/// Localize the rule-not-found message with the target and rule names.
fn rule_not_found_message(target_name: &str, rule_name: &str) -> localization::LocalizedMessage {
    let message = localization::message(keys::IR_RULE_NOT_FOUND);
    let with_target = add_arg(message, "target", target_name);
    add_arg(with_target, "rule", rule_name)
}

/// Find output paths that would collide with existing or sibling outputs.
#[cfg(kani)]
pub(super) fn find_duplicates(
    outputs: &[Utf8PathBuf],
    targets: &IrHashMap<Utf8PathBuf, EdgeId>,
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

/// Find output paths that would collide with the canonical graph index.
#[cfg(not(kani))]
fn find_duplicate_graph_outputs(
    outputs: &[Utf8PathBuf],
    graph: &BuildGraph,
) -> Option<Vec<Utf8PathBuf>> {
    let mut seen: Vec<&Utf8PathBuf> = Vec::new();
    let mut duplicates = Vec::new();
    for output in outputs {
        if graph.edge_id_for_output(output.as_path()).is_some()
            || sort_utils::has_seen_output(seen.as_slice(), output)
        {
            duplicates.push(output.clone());
        } else {
            seen.push(output);
        }
    }
    if duplicates.is_empty() {
        None
    } else {
        if duplicates.len() > 1 {
            sort_utils::sort_paths(&mut duplicates);
        }
        Some(duplicates)
    }
}

/// Return the first output path as the target's display name, or an empty
/// string when the target declares no explicit outputs.
pub(super) fn get_target_display_name(paths: &[Utf8PathBuf]) -> String {
    paths
        .first()
        .map_or_else(Default::default, |p: &Utf8PathBuf| p.to_string())
}

#[cfg(test)]
mod tests;
