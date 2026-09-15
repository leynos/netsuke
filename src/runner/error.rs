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
    /// Manifest evaluation exhausted an operator resource ceiling.
    #[error("{message}")]
    #[diagnostic(code(netsuke::runner::manifest_budget_exceeded))]
    ManifestBudgetExceeded {
        /// Bounded localized diagnostic without template text or context values.
        message: LocalizedMessage,
    },
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
        /// Localized error message.
        message: LocalizedMessage,
        /// Localized hint for resolving the error.
        #[help]
        help: LocalizedMessage,
    },
}

/// Promote budget exhaustion after command contexts have been attached.
///
/// Drop the enclosing error chain for this diagnostic because template errors
/// and command contexts can contain project-controlled or secret-bearing text.
pub(super) fn promote_manifest_budget(error: anyhow::Error) -> anyhow::Error {
    crate::manifest::budget_adapter::budget_exhaustion_message(&error).map_or(error, |message| {
        RunnerError::ManifestBudgetExceeded { message }.into()
    })
}

#[cfg(test)]
mod tests {
    //! Verify bounded budget promotion and unchanged unrelated diagnostics.

    use super::{RunnerError, promote_manifest_budget};
    use crate::manifest::{ManifestBudgetLimits, from_str_with_limits};

    #[test]
    fn promoted_budget_diagnostic_excludes_enclosing_secret_context() {
        let error = from_str_with_limits(
            "secret source",
            ManifestBudgetLimits {
                source_bytes: 1,
                ..ManifestBudgetLimits::default()
            },
        )
        .expect_err("source must exceed its budget")
        .context("secret outer context");
        let promoted = promote_manifest_budget(error);
        assert!(matches!(
            promoted.downcast_ref::<RunnerError>(),
            Some(RunnerError::ManifestBudgetExceeded { .. })
        ));
        assert!(!format!("{promoted:#}").contains("secret"));
        assert_eq!(promoted.chain().count(), 1);
    }

    #[test]
    fn unrelated_errors_keep_their_type_and_context() {
        let error = anyhow::Error::new(std::io::Error::other("original failure"))
            .context("original context");
        let promoted = promote_manifest_budget(error);
        assert!(promoted.downcast_ref::<std::io::Error>().is_some());
        assert_eq!(
            format!("{promoted:#}"),
            "original context: original failure"
        );
    }
}
