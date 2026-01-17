//! Error types for the runner module.
//!
//! This submodule isolates derive-macro-affected code to scope lint suppressions
//! narrowly. The `unused_assignments` lint fires in some Rust versions due to
//! thiserror/miette derive macro expansion.

// Scoped suppression for version-dependent lint false positives from
// miette/thiserror derive macros. The unused_assignments lint fires in some
// Rust versions but not others. Since `#[expect]` fails when the lint doesn't
// fire, and `unfulfilled_lint_expectations` cannot be expected, we must use
// `#[allow]` here.
// FIXME(rust-lang/rust#130021): remove once upstream is fixed.
#![allow(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    unused_assignments
)]

use crate::localization::LocalizedMessage;
use miette::Diagnostic;
use std::path::PathBuf;
use thiserror::Error;

/// Errors raised during command execution.
#[derive(Debug, Error, Diagnostic)]
pub enum RunnerError {
    /// The manifest file does not exist at the expected path.
    #[error("{message}")]
    #[diagnostic(code(netsuke::runner::manifest_not_found))]
    ManifestNotFound {
        /// Name of the expected manifest file (e.g., "Netsukefile").
        manifest_name: String,
        /// Directory description (e.g., "the current directory").
        directory: String,
        /// The path that was attempted.
        path: PathBuf,
        /// Localised error message.
        message: LocalizedMessage,
        /// Localised hint for resolving the error.
        #[help]
        help: LocalizedMessage,
    },
}
