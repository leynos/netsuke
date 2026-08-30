//! Validation for command-list boundaries before Ninja rendering.

use super::ninja_gen_command_list::{
    CommandListEntry, CommandListEntryError, command_list_entry_error,
};
use super::{NinjaGenError, ninja_gen_escape::validate_ninja_value};
use crate::ast::{Recipe, StringOrList};

/// Reject recipes the generated shell cannot execute with stable semantics.
pub(super) fn validate_action_recipe(
    action: &crate::ir::Action,
    action_index: usize,
) -> Result<(), NinjaGenError> {
    if let Recipe::Command { command } = &action.recipe
        && command.is_empty_content()
    {
        return Err(NinjaGenError::EmptyCommandRecipe { action_index });
    }
    if let Recipe::Command {
        command: StringOrList::List(entries),
    } = &action.recipe
    {
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
