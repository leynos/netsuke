//! Resolve manifest recipes into their interpolated command and script text.
//!
//! Commands and scripts share the action's prepared path bindings. A shared
//! PowerShell command list is additionally scanned as one script, because the
//! entries are parsed where their predecessors stopped.

use super::{ActionBindings, RecipeShell};
use crate::ast::{CommandIsolation, StringOrList};
use crate::ir::IrGenError;
use crate::ir::cmd_interpolate::{
    CommandBindings, PowerShellListCarry, interpolate_command_with_bindings,
    interpolate_script_with_bindings, interpolate_shared_power_shell_entry,
};

/// Resolves input and output bindings in every command representation.
pub(super) fn resolve_command(
    command: StringOrList,
    bindings: ActionBindings<'_>,
    isolation: CommandIsolation,
) -> Result<StringOrList, IrGenError> {
    let command_bindings = CommandBindings::new(bindings.inputs, bindings.outputs, bindings.shell);
    let is_shared_power_shell = bindings.shell == RecipeShell::PowerShell && isolation.is_shared();
    match command {
        StringOrList::String(scalar_command) => Ok(StringOrList::String(
            interpolate_command_with_bindings(&scalar_command, &command_bindings)?,
        )),
        StringOrList::List(items) if is_shared_power_shell => {
            resolve_shared_power_shell_list(items, &command_bindings)
        }
        StringOrList::List(items) => items
            .into_iter()
            .map(|item| interpolate_command_with_bindings(&item, &command_bindings))
            .collect::<Result<Vec<_>, _>>()
            .map(StringOrList::List),
        // An empty command list cannot deserialize (the manifest parser
        // rejects it), so nothing needs interpolating here.
        StringOrList::Empty => Ok(StringOrList::Empty),
    }
}

/// Resolve a shared PowerShell list by scanning entries where their predecessors ended.
///
/// The entries are parsed as one script by the executing interpreter, so each
/// entry inherits the previous entry's lexical boundary. An entry that leaves a
/// region open is rejected before its successor's standalone path literals can
/// be reclassified inside that region. Bindings are still prepared once per
/// list, and the resolved command text is unchanged for entries that end
/// neutrally.
fn resolve_shared_power_shell_list(
    items: Vec<String>,
    command_bindings: &CommandBindings,
) -> Result<StringOrList, IrGenError> {
    let mut carry = PowerShellListCarry::new();
    let mut resolved = Vec::with_capacity(items.len());
    for (index, item) in items.into_iter().enumerate() {
        let (interpolated, next) =
            interpolate_shared_power_shell_entry(&item, command_bindings, carry, index + 1)?;
        resolved.push(interpolated);
        carry = next;
    }
    Ok(StringOrList::List(resolved))
}

/// Resolves input and output bindings in a script without interpreting shell syntax.
pub(super) fn resolve_script(
    script: &str,
    bindings: ActionBindings<'_>,
) -> Result<String, IrGenError> {
    interpolate_script_with_bindings(
        script,
        &CommandBindings::new(bindings.inputs, bindings.outputs, bindings.shell),
    )
}
