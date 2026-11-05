//! Decide whether a merged registry of configured shells is admissible.
//!
//! Merging concatenates the operator layers' definitions in merge order, so the
//! merged sequence is the whole registry: nothing is replaced, keyed, or
//! deduplicated. This module then applies the two composition rules. No
//! definition may reuse a built-in spelling, because the built-ins are fixed
//! invocations rather than defaults a definition may shadow, and no two
//! definitions may share a name, because a name selects exactly one program.
//!
//! Identical repeats collide like any other pair. Treating a repeat as
//! idempotent would make the meaning of a definition depend on how many layers
//! happened to mention it, which is precisely the ambiguity the rule forbids.
//! An empty higher layer therefore cannot narrow the registry either: it
//! contributes no definitions and cannot remove any.
//!
//! Validation reports the first fault and stops. A partially accepted registry
//! would leave a caller holding definitions that were never checked as a whole.

use super::definition::ShellDefinition;
use crate::shell_selection::BuiltInShell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use thiserror::Error;

/// Validate the merged registry, reporting its first composition fault.
///
/// # Errors
///
/// Returns [`CompositionError::Reserved`] when a definition reuses a built-in
/// name, and [`CompositionError::Duplicate`] when two positions share a name.
///
/// # Examples
///
/// ```
/// use netsuke::cli::config::shells::{ShellArguments, ShellDefinition, ShellExecutable};
/// use netsuke::cli::config::shells::validate_composition;
/// use netsuke::shell_selection::{HostFamily, ShellName};
/// let definition = ShellDefinition {
///     name: ShellName::parse("dash")?,
///     executable: ShellExecutable::parse("dash", HostFamily::UnixLike)?,
///     args: ShellArguments::parse(&["-c".to_owned()])?,
/// };
/// assert!(validate_composition(std::slice::from_ref(&definition)).is_ok());
/// assert!(validate_composition(&[definition.clone(), definition]).is_err());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn validate_composition(definitions: &[ShellDefinition]) -> Result<(), CompositionError> {
    let mut first_seen: HashMap<&str, usize> = HashMap::with_capacity(definitions.len());
    for (index, definition) in definitions.iter().enumerate() {
        check_definition(definition, index, &mut first_seen)?;
    }
    Ok(())
}

/// Reject a built-in name, then reject a name already seen at an earlier index.
///
/// The built-in check runs first so a definition that shadows a built-in is
/// reported as such rather than as a duplicate of some other definition.
fn check_definition<'a>(
    definition: &'a ShellDefinition,
    index: usize,
    first_seen: &mut HashMap<&'a str, usize>,
) -> Result<(), CompositionError> {
    let name = definition.name.as_str();
    if BuiltInShell::lookup(&definition.name).is_some() {
        return Err(CompositionError::Reserved {
            index,
            name: name.to_owned(),
        });
    }
    match first_seen.entry(name) {
        Entry::Occupied(entry) => Err(CompositionError::Duplicate {
            index,
            first: *entry.get(),
            name: name.to_owned(),
        }),
        Entry::Vacant(entry) => {
            entry.insert(index);
            Ok(())
        }
    }
}

/// Classify the composition rules a merged registry may violate.
///
/// A name may be quoted verbatim in these messages: [`ShellName`] admits only
/// `[a-z][a-z0-9_-]{0,62}`, so the spelling can neither be arbitrarily long nor
/// contain a control character.
///
/// [`ShellName`]: crate::shell_selection::ShellName
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum CompositionError {
    /// Reject a definition that reuses a built-in shell name.
    #[error(
        "shells[{index}] names `{name}`, which is a built-in shell; \
         built-in names may not be redefined"
    )]
    Reserved {
        /// Report the position of the offending definition.
        index: usize,
        /// Report the reserved name that position declares.
        name: String,
    },
    /// Reject a second definition of a name already declared earlier.
    #[error(
        "shells[{index}] names `{name}`, which shells[{first}] already names; \
         each shell name may be declared once across the merged configuration"
    )]
    Duplicate {
        /// Report the position of the later definition.
        index: usize,
        /// Report the position of the definition that claimed the name first.
        first: usize,
        /// Report the name both positions declare.
        name: String,
    },
}
