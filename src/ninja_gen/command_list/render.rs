//! Render ordered command entries for the selected shell-state policy.

use crate::ast::CommandIsolation;

use super::super::escape::ShellText;
use super::super::{NamedAction, NinjaGenError, RecipeShell};
use super::{ActionId, CommandListEntry, command_list_entry};

/// Render a command list through PowerShell or the shared POSIX entry wrapper.
pub(in crate::ninja_gen) fn command_list_shell_text(
    action_id: &str,
    items: &[String],
    isolation: CommandIsolation,
    shell: RecipeShell,
) -> Result<ShellText, NinjaGenError> {
    let power_shell_script = match isolation {
        CommandIsolation::Shared => shell.command_list_script(items),
        CommandIsolation::Isolated => shell.isolated_command_list_script(items)?,
    };
    if let Some(script) = power_shell_script {
        return Ok(ShellText::new(script));
    }
    // Brace groups keep each entry a distinct shell unit, and `eval` prevents
    // comments or trailing control operators inside an entry consuming its
    // terminator. Shared groups run in the current shell; isolated groups run
    // in subshells. Both modes retain the fail-fast `&&` chain.
    let command_line = items
        .iter()
        .enumerate()
        .map(|(entry_index, item)| {
            let entry = command_list_entry(
                CommandListEntry(item),
                ActionId(action_id),
                entry_index + 1,
                isolation,
            );
            match isolation {
                CommandIsolation::Shared => entry,
                CommandIsolation::Isolated => format!("( {entry} )"),
            }
        })
        .collect::<Vec<_>>()
        .join(" && ");
    NamedAction::assert_shell_command(&command_line);
    Ok(ShellText::new(command_line))
}
