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

#[cfg(feature = "lint")]
use super::FindingDiagnostic;

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

    /// The selected Ninja executable could not be started.
    ///
    /// The OS error stays in the cause chain, so a reader sees both what was
    /// attempted and why it failed without the runner having to guess which of
    /// absence, permissions, or an invalid image applies.
    #[error("{message}")]
    #[diagnostic(code(netsuke::runner::ninja_spawn_failed))]
    NinjaSpawnFailed {
        /// The program exactly as configured, before citation escaping.
        program: String,
        /// Localized summary naming the attempted program.
        message: LocalizedMessage,
        /// Localized hint naming the installation and configuration remedies.
        #[help]
        help: LocalizedMessage,
        /// The launch failure reported by the operating system.
        #[source]
        source: std::io::Error,
    },

    /// Ninja ran but reported an unsuccessful exit status.
    ///
    /// The full status travels as the source, so a programmatic caller can tell
    /// an ordinary exit code from a Unix signal, and any command-list
    /// attribution stays readable in the cause chain.
    #[error("{message}")]
    #[diagnostic(code(netsuke::runner::ninja_failed))]
    NinjaFailed {
        /// The status Ninja exited with, signals included.
        status: std::process::ExitStatus,
        /// Invocation that failed: `build` or the Ninja tool name.
        operation: String,
        /// Localized summary naming the status.
        message: LocalizedMessage,
        /// The exit failure, carrying the legacy text and any attribution.
        #[source]
        source: super::NinjaExitError,
    },

    /// The graph-bearing help catalogue requires machine-readable output.
    #[error("{message}")]
    #[diagnostic(code(netsuke::runner::help_targets_edges_require_json))]
    HelpTargetsEdgesRequireJson {
        /// Localized diagnostic explaining the required root flag.
        message: LocalizedMessage,
    },

    /// Lint findings reached the configured failure threshold.
    ///
    /// The findings travel as related diagnostics rather than as a formatted
    /// message so that both the human renderer and the JSON serializer see the
    /// same per-finding objects, each with its own code, severity, help text,
    /// documentation URL, and source span.
    #[cfg(feature = "lint")]
    #[error("{message}")]
    #[diagnostic(code(netsuke::lint::threshold_exceeded))]
    LintThresholdExceeded {
        /// Localized summary naming the threshold and the finding counts.
        message: LocalizedMessage,
        /// Localized hint for resolving the failure.
        #[help]
        help: LocalizedMessage,
        /// Every reported finding, in output order.
        #[related]
        findings: Vec<FindingDiagnostic>,
    },

    /// A `netsuke check` policy selector could not be applied.
    #[cfg(feature = "lint")]
    #[error("{message}")]
    #[diagnostic(code(netsuke::lint::invalid_policy))]
    CheckPolicy {
        /// Localized description of the rejected selector.
        message: LocalizedMessage,
    },

    /// The manifest source could not be indexed for lint diagnostics.
    #[cfg(feature = "lint")]
    #[error("{message}")]
    #[diagnostic(code(netsuke::lint::source_index_failed))]
    CheckSourceIndex {
        /// Localized description of where indexing stopped.
        message: LocalizedMessage,
    },
}

/// Promote budget exhaustion after command contexts have been attached.
///
/// Drop the enclosing error chain for this diagnostic because template errors
/// and command contexts can contain project-controlled or secret-bearing text.
pub(super) fn promote_manifest_budget(error: anyhow::Error) -> anyhow::Error {
    crate::manifest::budget_exhaustion_message(&error).map_or(error, |message| {
        RunnerError::ManifestBudgetExceeded { message }.into()
    })
}

#[cfg(test)]
mod tests {
    //! Verify bounded budget promotion and unchanged unrelated diagnostics.

    use super::{RunnerError, promote_manifest_budget};
    use crate::manifest::{ManifestBudgetLimits, from_str_with_limits};
    use test_support::fluent::normalize_fluent_isolates;

    #[test]
    fn promoted_collection_exhaustion_keeps_its_stage_and_redacts_the_template() {
        let yaml = concat!(
            "netsuke_version: 1.0.0\n",
            "targets:\n",
            "  - name: s3cr3t-target\n",
            "    command: \"{{ (['s3cr3t-value'] * 100001) | compact | length }}\"\n",
        );
        let error = from_str_with_limits(yaml, ManifestBudgetLimits::default())
            .expect_err("the repetition must exceed the per-call collection ceiling")
            .context("s3cr3t outer context");
        let promoted = promote_manifest_budget(error);

        let diagnostic = promoted
            .downcast_ref::<RunnerError>()
            .expect("collection exhaustion must promote to the typed runner variant");
        assert!(
            matches!(diagnostic, RunnerError::ManifestBudgetExceeded { .. }),
            "unexpected runner variant for a collection failure"
        );
        assert_eq!(
            miette::Diagnostic::code(diagnostic)
                .map(|code| code.to_string())
                .as_deref(),
            Some("netsuke::runner::manifest_budget_exceeded"),
            "the promoted diagnostic must keep its stable JSON code"
        );

        let rendered = normalize_fluent_isolates(&format!("{promoted:#}"));
        assert!(
            rendered.contains("during collection"),
            "the promoted diagnostic must keep the collection stage: {rendered}"
        );
        assert!(
            !rendered.contains("s3cr3t"),
            "the promoted diagnostic must not echo manifest text: {rendered}"
        );
        assert_eq!(
            promoted.chain().count(),
            1,
            "promotion must drop the enclosing template context"
        );
    }

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
