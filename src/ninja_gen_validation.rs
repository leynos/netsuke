//! Validate recipes and metadata before Ninja rendering.

use super::ninja_gen_command_list::{
    CommandListEntry, CommandListEntryError, command_list_entry_error,
};
use super::{NinjaGenError, RecipeShell, ninja_gen_escape::validate_ninja_value};
use crate::ast::{Recipe, StringOrList};
/// Reject recipes the generated shell cannot execute with stable semantics.
pub(super) fn validate_action_recipe(
    action: &crate::ir::Action,
    action_index: usize,
    shell: RecipeShell,
) -> Result<(), NinjaGenError> {
    if let Recipe::Command { command } = &action.recipe
        && command.is_empty_content()
    {
        return Err(NinjaGenError::EmptyCommandRecipe { action_index });
    }
    if shell == RecipeShell::PowerShell {
        return Ok(());
    }
    match &action.recipe {
        Recipe::Command {
            command: StringOrList::String(command),
        } => validate_scalar_command(command),
        Recipe::Command {
            command: StringOrList::List(entries),
        } => validate_posix_command_list(entries, action_index),
        Recipe::Command {
            command: StringOrList::Empty,
        }
        | Recipe::Script { .. }
        | Recipe::Rule { .. } => Ok(()),
    }
}

/// Reject scalar command text that cannot occupy one Ninja command binding.
fn validate_scalar_command(command: &str) -> Result<(), NinjaGenError> {
    validate_ninja_value(command)
}

/// Reject POSIX command-list entries that violate the generated wrapper contract.
fn validate_posix_command_list(
    entries: &[String],
    action_index: usize,
) -> Result<(), NinjaGenError> {
    for (zero_based_entry_index, entry) in entries.iter().enumerate() {
        let entry_index = zero_based_entry_index + 1;
        match command_list_entry_error(CommandListEntry(entry)) {
            Some(CommandListEntryError::MultipleBackgroundJobs) => {
                return Err(NinjaGenError::MultipleBackgroundJobs {
                    action_index,
                    entry_index,
                });
            }
            Some(CommandListEntryError::UnsupportedExec) => {
                return Err(NinjaGenError::UnsupportedCommandListExec {
                    action_index,
                    entry_index,
                });
            }
            Some(CommandListEntryError::UnanalyzableEval) => {
                return Err(NinjaGenError::UnanalyzableCommandListEval {
                    action_index,
                    entry_index,
                });
            }
            Some(CommandListEntryError::NinjaControlCharacter) => {
                return Err(NinjaGenError::NinjaControlCharacter {
                    action_index,
                    entry_index,
                });
            }
            None => {}
        }
    }
    Ok(())
}
/// Reject metadata values that cannot remain within one Ninja binding.
pub(super) fn validate_action_metadata(action: &crate::ir::Action) -> Result<(), NinjaGenError> {
    for value in [
        action.description.as_ref(),
        action.depfile.as_ref(),
        action.deps_format.as_ref(),
        action.pool.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_ninja_value(value)?;
    }
    Ok(())
}
