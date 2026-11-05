//! Human-readable rendering for the failures a run can end with.
//!
//! The JSON path already picks the most specific diagnostic out of a failure
//! chain: a [`runner::RunnerError`], then a [`manifest::ManifestError`], then an
//! [`miette::Report`]. The human path special-cased only the first of those, so
//! a manifest rejection printed its contextual summary — "Failed to load
//! manifest at …" — and dropped the diagnostic saying what was actually wrong.
//!
//! A rendered diagnostic replaces the contextual summary rather than following
//! it: the diagnostic already names the manifest path, so printing both repeats
//! it. A failure carrying no diagnostic keeps its summary line, which is the
//! whole report for errors such as a missing manifest. Either way a person sees
//! the same fault the JSON mode reports, at the same depth.
//!
//! Nothing here reads the process environment. The glyph theme is derived from
//! the already-resolved [`OutputPrefs`], which keeps this module callable from
//! an in-process test without ambient state.

use anyhow::Error;
use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme, Report};
use netsuke::ir::IrGenError;
use netsuke::manifest;
use netsuke::output_prefs::OutputPrefs;
use netsuke::runner;
use std::io::{self, Write};
use std::process::ExitCode;

/// Choose the glyph theme that matches the resolved output preferences.
///
/// The theme is monochrome in both arms. Colour is deliberately left to the
/// caller's terminal conventions rather than to ANSI escapes embedded in a
/// diagnostic, which keeps piped and redirected stderr greppable.
#[must_use]
pub fn theme_for(prefs: OutputPrefs) -> GraphicalTheme {
    if prefs.emoji_allowed() {
        GraphicalTheme::unicode_nocolor()
    } else {
        GraphicalTheme::none()
    }
}

/// Render a failure for a person, or `None` when it carries no diagnostic.
///
/// Selection mirrors the JSON path, so both modes report the same fault. The
/// graphical layout carries the diagnostic's code, its cause chain, and its
/// help text. A failure carrying none of these diagnostics returns `None`,
/// because its context message is the whole report the human path has.
#[must_use]
pub fn render_failure(err: &Error, theme: &GraphicalTheme) -> Option<String> {
    if let Some(runner_err) = err.downcast_ref::<runner::RunnerError>() {
        return Some(render_diagnostic(runner_err, theme));
    }
    if let Some(manifest_err) = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<manifest::ManifestError>())
    {
        return Some(render_diagnostic(manifest_err, theme));
    }
    let report = err.downcast_ref::<Report>()?;
    Some(render_diagnostic(&**report, theme))
}

/// Find a circular-dependency error inside a contextual runner failure.
#[must_use]
pub fn circular_dependency_cause(error: &Error) -> Option<&IrGenError> {
    error.chain().find_map(|cause| {
        cause
            .downcast_ref::<IrGenError>()
            .filter(|ir_error| matches!(ir_error, IrGenError::CircularDependency { .. }))
    })
}

/// Report a runner failure on stderr and return the failure exit code.
///
/// A rendered diagnostic replaces the context line rather than following it:
/// the diagnostic already names the path, so printing both repeats it. A
/// failure with no diagnostic keeps the context line, which is how these
/// failures have always been reported. A cycle keeps its own branch, because
/// its bounded rendering is what keeps manifest target names out of the tracing
/// event and the diagnostic adds nothing a reader needs.
///
/// The glyph theme is derived here from `prefs` rather than passed in, because
/// every caller has the preferences already and the two must not disagree.
pub fn handle_failure(err: &Error, prefs: &OutputPrefs) -> ExitCode {
    let prefix = prefs.error_prefix();
    if let Some(cycle_error) = circular_dependency_cause(err) {
        tracing::error!(error_kind = "circular_dependency", "runner failed");
        drop(writeln!(io::stderr(), "{prefix} {cycle_error}"));
        return ExitCode::FAILURE;
    }
    tracing::error!(error_kind = failure_kind(err), "runner failed");
    match render_failure(err, &theme_for(*prefs)) {
        Some(rendered) => drop(writeln!(io::stderr(), "{prefix} {rendered}")),
        None => drop(writeln!(io::stderr(), "{prefix} {err}")),
    }
    ExitCode::FAILURE
}

/// Classify a runner failure into the fixed category logged at this boundary.
///
/// A schema rejection is distinguished from every other runner failure because
/// it is the only one an operator can fix by editing the manifest's version
/// declaration rather than its build description. The rendering is unchanged;
/// only the logged category differs, and the diagnostic it accompanies still
/// carries the full explanation.
fn failure_kind(err: &Error) -> &'static str {
    let is_schema_rejection = err.chain().any(|cause| {
        cause
            .downcast_ref::<manifest::ManifestError>()
            .is_some_and(|manifest_error| {
                ManifestFailure::of(manifest_error).is_schema_version_rejection()
            })
    });
    if is_schema_rejection {
        "manifest_schema_version"
    } else {
        "runner"
    }
}

/// Fixed classification of a manifest failure for the human boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ManifestFailure {
    /// The failure carries the schema-version diagnostic code.
    SchemaVersionRejection,
    /// The failure is any other manifest fault.
    Other,
}

impl ManifestFailure {
    /// Classify a manifest error by its diagnostic source's stable code.
    ///
    /// The code is read rather than the message, because the message is
    /// localized and one option carries the declared version text.
    fn of(error: &manifest::ManifestError) -> Self {
        match error {
            manifest::ManifestError::Parse { source, .. }
                if source.code().is_some_and(|code| {
                    code.to_string() == "netsuke::manifest::schema_version"
                }) =>
            {
                Self::SchemaVersionRejection
            }
            manifest::ManifestError::Parse { .. } => Self::Other,
        }
    }

    /// Report whether this is a schema-version rejection.
    const fn is_schema_version_rejection(self) -> bool {
        matches!(self, Self::SchemaVersionRejection)
    }
}

/// Render one diagnostic with `miette`'s graphical layout.
///
/// Links are disabled because `miette` emits OSC 8 hyperlinks whenever a
/// diagnostic supplies a [`Diagnostic::url`], and that happens even under a
/// monochrome theme. A `netsuke check` finding always supplies one, so leaving
/// links on would put terminal control sequences into redirected stderr. The
/// URL is still printed as plain text after the code, so nothing is lost from
/// a transcript.
fn render_diagnostic(diagnostic: &dyn Diagnostic, theme: &GraphicalTheme) -> String {
    let mut rendered = String::new();
    let handler = GraphicalReportHandler::new_themed(theme.clone()).with_links(false);
    // Rendering into a `String` cannot fail, so this would be a bug in `miette`
    // rather than a fault worth reporting. The diagnostic's own summary is the
    // fallback, and it still names the problem.
    if handler.render_report(&mut rendered, diagnostic).is_err() {
        return diagnostic.to_string();
    }
    rendered
}

#[cfg(test)]
mod tests;
