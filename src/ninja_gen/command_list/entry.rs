//! Helpers for identifying and quoting one ordered command-list entry.

use sha2::{Digest, Sha256};

use crate::hex::to_lower_hex;

use super::{ActionId, CommandListEntry};

/// Return a fixed-width fingerprint for an action identifier.
///
/// IR-generated identifiers are already hashes, but hashing again prevents a
/// programmatically supplied identifier from disclosing arbitrary content.
pub(super) fn action_identity(action_id: ActionId<'_>) -> String {
    to_lower_hex(&Sha256::digest(action_id.0.as_bytes()))
}

/// Quote `value` as one literal POSIX shell argument.
///
/// The command-list renderer passes each entry to `eval` so an inline comment
/// or trailing control operator cannot consume the brace-group terminator.
pub(super) fn shell_single_quote(command: CommandListEntry<'_>) -> String {
    // `shell_quote::QuoteRefExt::quoted(Sh)` produces minimally quoted
    // fragments, while the `eval` wrapper requires this canonical enclosing
    // form to preserve its generated Ninja text and delimiter contract.
    let escaped = command.0.replace('\'', r"'\''");
    format!("'{escaped}'")
}
