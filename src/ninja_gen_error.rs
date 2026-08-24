//! Errors produced while rendering Ninja manifests.

use crate::localization::{self, LocalizedMessage, keys};
use std::fmt;
use thiserror::Error;

/// Errors produced while rendering Ninja manifests.
#[derive(Debug, Error)]
pub enum NinjaGenError {
    /// The build graph referenced an action that was not defined.
    #[error("{message}")]
    MissingAction {
        /// Identifier of the missing action referenced by a build edge.
        id: String,
        /// Localized error message.
        message: LocalizedMessage,
    },
    /// An action built outside manifest deserialization has no command entries.
    #[error("command-list action {action_index} has no command entries")]
    EmptyCommandRecipe {
        /// One-based stable position in generated action order.
        action_index: usize,
    },
    /// A list entry starts multiple background jobs, which cannot be
    /// attributed reliably by a shared POSIX shell.
    #[error(
        "command-list action {action_index}, entry {entry_index} has unsupported background jobs"
    )]
    MultipleBackgroundJobs {
        /// One-based stable position in generated action order.
        action_index: usize,
        /// One-based stable position in the command list.
        entry_index: usize,
    },
    /// A list entry uses `exec` in a shell structure the wrapper cannot
    /// supervise without changing its semantics.
    #[error(
        "command-list action {action_index}, entry {entry_index} has unsupported exec structure"
    )]
    UnsupportedCommandListExec {
        /// One-based stable position in generated action order.
        action_index: usize,
        /// One-based stable position in the command list.
        entry_index: usize,
    },
    /// A list entry contains a dynamic `eval` payload whose background jobs
    /// cannot be attributed reliably.
    #[error(
        "command-list action {action_index}, entry {entry_index} has an unanalyzable eval payload"
    )]
    UnanalyzableCommandListEval {
        /// One-based stable position in generated action order.
        action_index: usize,
        /// One-based stable position in the command list.
        entry_index: usize,
    },
    /// A list entry contains a control character that cannot be serialized in
    /// one Ninja command binding.
    #[error(
        "command-list action {action_index}, entry {entry_index} contains an unsafe Ninja control character"
    )]
    NinjaControlCharacter {
        /// One-based stable position in generated action order.
        action_index: usize,
        /// One-based stable position in the command list.
        entry_index: usize,
    },
    /// A graph with serial dependencies cannot be represented by a single
    /// build-file string; callers must use [`crate::ninja_gen::generate_bundle`].
    #[error("{message}")]
    DyndepFilesRequired {
        /// Localized error message.
        message: LocalizedMessage,
    },
    /// A user graph path collides with Netsuke's reserved state namespace.
    #[error("{message}")]
    ReservedOutputPath {
        /// Colliding path.
        path: camino::Utf8PathBuf,
        /// Localized error message.
        message: LocalizedMessage,
    },
    /// A path contains a character unsupported by Ninja path syntax.
    #[error("{message}")]
    UnsupportedPathCharacter {
        /// Path containing the unsupported character.
        path: camino::Utf8PathBuf,
        /// Unsupported character.
        character: char,
        /// Localized error message.
        message: LocalizedMessage,
    },
    /// A scalar command or script cannot be represented in one Ninja binding.
    #[error("Ninja binding contains an unsafe control character")]
    UnsafeNinjaValue,
    /// A path cannot be represented consistently in a Ninja build edge.
    #[error("Ninja path contains an unsafe character: {path}")]
    UnsafeNinjaPath {
        /// Path rejected before its build edge reaches the generated file.
        path: String,
    },
    /// Formatting the Ninja output failed.
    #[error("{message}")]
    Format {
        /// Underlying formatting error.
        #[source]
        source: fmt::Error,
        /// Localized error message.
        message: LocalizedMessage,
    },
}

impl From<fmt::Error> for NinjaGenError {
    fn from(source: fmt::Error) -> Self {
        Self::Format {
            message: localization::message(keys::NINJA_GEN_FORMAT),
            source,
        }
    }
}
