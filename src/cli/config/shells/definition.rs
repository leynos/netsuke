//! Define one trusted shell definition and the exact shape it may take.
//!
//! A definition carries precisely three fields. Rejecting unknown fields rather
//! than ignoring them keeps a mistyped key from silently falling back to a
//! default: an operator who writes `executabl` must be told, not quietly given a
//! definition that omits the program it names.
//!
//! The field types do the validating. Each is deserialized through its own
//! checked representation, so a definition that exists at all is already valid
//! and nothing downstream has to re-check a field it receives.

use super::arguments::ShellArguments;
use super::executable::ShellExecutable;
use crate::shell_selection::ShellName;
use serde::{Deserialize, Serialize};

/// Represent one configured shell as the operator declared it.
///
/// The definition is data, not authority: it says which program a shell name
/// would run and with which arguments, and nothing here executes that program.
/// See ADR-0019 for why a definition is trusted operator policy rather than a
/// sandbox.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShellDefinition {
    /// Name the adapter selects this shell by, subject to the reserved words.
    pub name: ShellName,
    /// Name the program to run, as a bare name or an absolute path.
    pub executable: ShellExecutable,
    /// Pass these arguments to the program, in this order.
    pub args: ShellArguments,
}
