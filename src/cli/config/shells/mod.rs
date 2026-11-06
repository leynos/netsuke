//! Validate trusted configured-shell definitions.
//!
//! A definition declares a shell name, the program that name would run, and the
//! arguments that program would receive. Definitions are operator policy: only
//! a system or user operator layer may declare them, because a definition
//! decides what Netsuke would execute. Structural checking can show that a
//! definition is well formed; it cannot prove that the program a definition
//! names is harmless. A mistaken or hostile operator definition is therefore an
//! operator policy error rather than something this module can contain. See
//! ADR-0019 for the trust boundary, and note that nothing here executes a
//! configured program.
//!
//! # Composition
//!
//! Layers merge by concatenation in merge order, so the merged sequence is the
//! registry. Nothing is replaced, keyed, or deduplicated, and an empty higher
//! layer neither clears nor narrows a lower one. [`validate_composition`] then
//! rejects a built-in spelling and any name declared twice, including an
//! identical repeat. Validation runs once, after merging, so a failure yields
//! no partial registry.
//!
//! # Boundaries
//!
//! Field types validate as they deserialize, so a [`ShellDefinition`] that
//! exists is already locally valid. That is what lets the same field
//! deserializers both check a single occurrence during discovery and check the
//! merged vector without duplicating any rule.

/// Top-level configuration key that declares shell definitions.
///
/// The schema owns the key so that discovery, the environment layer, and this
/// module's own diagnostics all name the same spelling. Keeping it here rather
/// than in `cli::constants` also lets the build script's compiled slice carry
/// it without pulling in the modules that share that constants file.
pub const SHELLS_KEY: &str = "shells";

mod arguments;
mod compose;
mod definition;
mod executable;
pub(crate) mod preview;

pub use arguments::{ArgumentsError, MAX_ARGUMENT_BYTES, MAX_ARGUMENTS, ShellArguments};
pub use compose::{CompositionError, validate_composition};
pub use definition::ShellDefinition;
pub use executable::{ExecutableError, ShellExecutable};

#[cfg(test)]
mod tests;
