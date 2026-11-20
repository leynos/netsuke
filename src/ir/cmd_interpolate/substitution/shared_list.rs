//! Interpolate shared PowerShell command-list entries as one script.
//!
//! A shared list is joined into a single script, and the entries are separated
//! only by wrapper statements that reset and check `$LASTEXITCODE`. That means
//! PowerShell parses entry `n + 1` where entry `n` stopped. Interpolating each
//! entry in isolation is therefore unsound: an entry that leaves a string open
//! makes the next entry's standalone path literal part of that string, and can
//! turn filename text into executable syntax. Lowering scans entries in order,
//! refuses an entry that does not end at a neutral boundary, and carries the
//! fail-closed classification forward.

use super::super::{CommandBindings, IrGenError};
use super::SubstitutionTraversal;
use super::powershell::{InterpolationBoundary, PowerShellUnfinishedRegion};
use crate::localization::{self, keys};

/// Carry the shared-script lexical outcome from one entry to the next.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PowerShellListCarry {
    /// Record whether this entry or an earlier one needed fail-closed rejection.
    is_fail_closed: bool,
}

impl PowerShellListCarry {
    /// Start a shared list whose first entry inherits no earlier state.
    pub(crate) const fn new() -> Self {
        Self {
            is_fail_closed: false,
        }
    }

    /// Report whether later entries must keep rejecting unquoted markers.
    pub(crate) const fn is_fail_closed(self) -> bool {
        self.is_fail_closed
    }

    /// Continue the list after one entry's boundary summary.
    const fn from_boundary(boundary: InterpolationBoundary) -> Self {
        Self {
            is_fail_closed: boundary.is_fail_closed(),
        }
    }
}

/// Interpolate one entry of a shared PowerShell command list.
///
/// `entry_index` is the entry's one-based position, which the rejection
/// diagnostic names. The returned carry must be passed to the next entry.
///
/// # Errors
///
/// Returns [`IrGenError::InvalidCommand`] when the entry leaves a quoted
/// string, here-string, block comment, or command substitution open, or ends in
/// a backtick escape that would consume text from the following entry.
pub(crate) fn interpolate_shared_power_shell_entry(
    template: &str,
    bindings: &CommandBindings,
    carry: PowerShellListCarry,
    entry_index: usize,
) -> Result<(String, PowerShellListCarry), IrGenError> {
    let chars: Vec<char> = template.chars().collect();
    let mut traversal = SubstitutionTraversal::with_power_shell_fail_closed(
        template,
        &chars,
        bindings,
        carry.is_fail_closed(),
    );
    let mut pos = 0;
    while pos < chars.len() {
        pos = traversal.append_substitution_at_position(pos)?;
    }
    let (interpolated, boundary) = traversal.finish_with_boundary();
    if let Some(region) = boundary.unfinished_region() {
        return Err(unfinished_power_shell_region_error(
            template,
            entry_index,
            region,
        ));
    }
    Ok((interpolated, PowerShellListCarry::from_boundary(boundary)))
}

/// Build the diagnostic for an entry that leaves a lexical region open.
///
/// The message names the entry and the region but deliberately omits
/// interpolated path text: the offending entry is reported from its original
/// template, which is what the author can act on.
fn unfinished_power_shell_region_error(
    template: &str,
    entry_index: usize,
    region: PowerShellUnfinishedRegion,
) -> IrGenError {
    let region_code = region.code();
    let message = localization::message(keys::IR_UNFINISHED_POWER_SHELL_REGION)
        .with_arg("entry", entry_index)
        .with_arg("region", region_code);
    IrGenError::InvalidCommand {
        command: template.to_owned(),
        snippet: region_code.to_owned(),
        message,
    }
}

/// Report whether a shared-list fragment ends at a neutral PowerShell boundary.
///
/// Test-only: the shared-script wrapper text is emitted by the Ninja backend,
/// so a contract test needs to confirm that the wrapper itself neither leaves a
/// region open nor forces fail-closed rejection.
#[cfg(test)]
pub(crate) fn shared_power_shell_entry_is_neutral(template: &str) -> bool {
    let bindings = CommandBindings::new(&[], &[], crate::recipe_shell::RecipeShell::PowerShell);
    interpolate_shared_power_shell_entry(template, &bindings, PowerShellListCarry::new(), 1)
        .is_ok_and(|(_, carry)| !carry.is_fail_closed())
}
