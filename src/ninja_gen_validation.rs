//! Validate recipes and metadata before Ninja rendering.

use super::ninja_gen_command_list::{
    CommandListEntry, CommandListEntryError, command_list_entry_error,
};
use super::{NinjaGenError, RecipeShell, ninja_gen_escape::validate_ninja_value};
use crate::ast::{Recipe, StringOrList};


//! Validation for command-list boundaries before Ninja rendering.
};

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

#[cfg(test)]
mod tests {
    //! Verifies shell-specific recipe validation boundaries.

    use super::{NinjaGenError, RecipeShell, validate_action_recipe};
    use crate::ast::{Recipe, StringOrList};
    use crate::ir::Action;

    /// Construct one action with a supplied command recipe for validation tests.
    fn command_action(command: StringOrList) -> Action {
        Action {
            recipe: Recipe::Command { command },
            description: None,
            depfile: None,
            deps_format: None,
            pool: None,
            restat: false,
        }
    }

    /// Accept multi-line scalar text because PowerShell receives it through its transport.
    #[test]
    fn power_shell_accepts_a_multi_line_scalar_command() {
        let action = command_action(StringOrList::String(
            "Write-Output first\nWrite-Output second".into(),
        ));
        assert!(validate_action_recipe(&action, 1, RecipeShell::PowerShell).is_ok());
    }

    /// Bypass POSIX-only command-list restrictions for PowerShell command lists.
    #[test]
    fn power_shell_bypasses_posix_command_list_validation() {
        let action = command_action(StringOrList::List(vec!["echo one & echo two &".into()]));
        assert!(validate_action_recipe(&action, 1, RecipeShell::PowerShell).is_ok());
        assert!(matches!(
            validate_action_recipe(&action, 1, RecipeShell::Posix),
            Err(NinjaGenError::MultipleBackgroundJobs { .. })
        ));
    }

    /// Reject empty commands before applying the PowerShell validation bypass.
    #[test]
    fn power_shell_rejects_an_empty_command_recipe() {
        let action = command_action(StringOrList::Empty);
        assert!(matches!(
            validate_action_recipe(&action, 1, RecipeShell::PowerShell),
            Err(NinjaGenError::EmptyCommandRecipe { .. })
        ));
    }
}
