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
//! Nothing here reads the process environment. The glyph and colour theme is
//! derived from the already-resolved [`DiagnosticStyle`] and the localized
//! prefixes from [`OutputPrefs`], which keeps this module callable from an
//! in-process test without ambient state.
//!
//! A failure the runner classified itself — a [`runner::RunnerError`] — is
//! rendered but not logged: its diagnostic is the whole report, and a tracing
//! event would prepend a timestamped line to text that already reads as a
//! complete answer. That is why a missing manifest prints one line and not
//! two.

use anyhow::Error;
use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme, Report};
use netsuke::display_plan::DiagnosticStyle;
use netsuke::ir::IrGenError;
use netsuke::manifest;
use netsuke::output_prefs::OutputPrefs;
use netsuke::runner;
use std::io::{self, Write};
use std::process::ExitCode;

/// Choose the glyph and colour theme that matches the resolved style.
///
/// `miette`'s four predefined themes are the cross product of its two
/// independent decisions, so the style's `colour` and `unicode` flags select
/// one directly. Colour is not left to the terminal: the caller has already
/// decided it from the colour policy and the stream's terminal facts, and a
/// theme that ignored that would paint ANSI into a redirected stream.
///
/// Links are not a theme property. They are disabled on the handler instead,
/// because the style's `links` flag has to reach [`GraphicalReportHandler`]
/// rather than the glyph set.
#[must_use]
pub fn theme_for(style: DiagnosticStyle) -> GraphicalTheme {
    match (style.colour, style.unicode) {
        (true, true) => GraphicalTheme::unicode(),
        (true, false) => GraphicalTheme::ascii(),
        (false, true) => GraphicalTheme::unicode_nocolor(),
        (false, false) => GraphicalTheme::none(),
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
    Some(render_diagnostic(select_diagnostic(err)?, theme))
}

/// Choose the diagnostic the human path reports, or `None` if there is none.
///
/// The order is the JSON path's: a [`runner::RunnerError`], then the first
/// [`manifest::ManifestError`], then a [`Report`]. Both modes and both human
/// decisions read this one function, so they cannot disagree about which error
/// a report is about.
fn select_diagnostic(err: &Error) -> Option<&dyn Diagnostic> {
    if let Some(runner_err) = err.downcast_ref::<runner::RunnerError>() {
        return Some(runner_err);
    }
    if let Some(manifest_err) = manifest_cause(err) {
        return Some(manifest_err);
    }
    err.downcast_ref::<Report>()
        .map(|report| &**report as &dyn Diagnostic)
}

/// Find the first manifest error in a failure chain.
///
/// Rendering and classification must agree on which error they describe, so
/// both read the chain through this one lookup rather than each walking it.
/// First match wins: a later `ManifestError` describes an inner failure, not
/// the one whose diagnostic the human path renders.
fn manifest_cause(err: &Error) -> Option<&manifest::ManifestError> {
    err.chain()
        .find_map(|cause| cause.downcast_ref::<manifest::ManifestError>())
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
/// The theme is derived here from `presentation` rather than passed in, because
/// every caller has the resolved preferences already and the rendered result
/// must not disagree with the rest of the run's output.
///
/// A failure the runner classified itself is not logged. Its diagnostic is the
/// whole report, and a tracing event would prepend a timestamped line to text
/// that is already a complete, readable answer — the near-ubiquitous missing
/// manifest included. Only a failure the runner did not anticipate is logged,
/// because that one may reach an operator through logs alone.
pub fn handle_failure(err: &Error, presentation: Presentation) -> ExitCode {
    let prefix = presentation.prefs.error_prefix();
    if let Some(cycle_error) = circular_dependency_cause(err) {
        tracing::error!(error_kind = "circular_dependency", "runner failed");
        drop(writeln!(io::stderr(), "{prefix} {cycle_error}"));
        return ExitCode::FAILURE;
    }
    if !is_expected_failure(err) {
        tracing::error!(error_kind = failure_kind(err), "runner failed");
    }
    let theme = theme_for(presentation.style);
    match failure_report(err, &theme) {
        Some(report) => drop(writeln!(io::stderr(), "{prefix} {report}")),
        None => drop(writeln!(io::stderr(), "{prefix} {err}")),
    }
    ExitCode::FAILURE
}

/// The resolved presentation a failure is rendered under.
///
/// The two fields come from one [`DisplayPlan`](netsuke::display_plan::DisplayPlan)
/// and are consumed together on every path, so they travel as one value rather
/// than as two arguments a call site could transpose or mismatch.
#[derive(Debug, Clone, Copy)]
pub struct Presentation {
    /// Localized prefixes and glyph preferences.
    pub prefs: OutputPrefs,
    /// Colour, glyph, and link capabilities for the diagnostic itself.
    pub style: DiagnosticStyle,
}

/// Report whether the runner already classified this failure.
///
/// A [`runner::RunnerError`] is one the runner raises deliberately: it carries
/// its own diagnostic code, help text, and localized message, so it needs no
/// log event to be understood. Every other failure is unexpected, and logging
/// it is how it reaches an operator who only has the logs.
fn is_expected_failure(err: &Error) -> bool {
    err.downcast_ref::<runner::RunnerError>().is_some()
}

/// Compose the human report for a failure, or `None` when it has no diagnostic.
///
/// A rendered diagnostic replaces the context line only when it visibly names
/// the artefact itself. A schema-version rejection names the declaration and
/// the supported schema but never which Netsukefile was refused, so it keeps
/// the context line; a YAML diagnostic draws a labelled snippet of the file and
/// so does not repeat it.
///
/// "Names the artefact" is [`names_its_own_source`], not merely
/// [`Diagnostic::source_code`]: a source with no label prints no snippet, so
/// the file never appears. Judging on the source alone would drop the context
/// line for a diagnostic that never showed the path, and the failure would name
/// no file at all.
fn failure_report(err: &Error, theme: &GraphicalTheme) -> Option<String> {
    let rendered = render_failure(err, theme)?;
    let diagnostic = select_diagnostic(err)?;
    if names_its_own_source(diagnostic) || repeats_the_message(err, diagnostic) {
        return Some(rendered);
    }
    Some(format!("{err}\n{rendered}"))
}

/// Report whether the context line merely restates the diagnostic's own text.
///
/// A top-level [`runner::RunnerError`] *is* the selected diagnostic, so its
/// context line and its first rendered cause line are the same sentence. Keeping
/// both would print it twice and push the diagnostic code — the line a reader
/// greps for — below the duplicate. A context line earns its place only when it
/// says something the rendering does not, which is exactly when the prose
/// differs.
fn repeats_the_message(err: &Error, diagnostic: &dyn Diagnostic) -> bool {
    err.to_string() == diagnostic.to_string()
}

/// Report whether a diagnostic renders a snippet naming its own file.
///
/// `miette` draws a snippet only where a diagnostic supplies *both* a source
/// and at least one label: `render_snippets` returns early when there are no
/// labels. A source with no label therefore prints nothing, so judging on the
/// source alone would drop the path from the report entirely.
///
/// The pair is looked for on the selected diagnostic and then down its
/// [`Diagnostic::diagnostic_source`] chain, because the wrapper that carries the
/// snippet is rarely the one selected. A manifest parse failure selects
/// [`manifest::ManifestError`], whose `#[source_code]` is absent and whose
/// `#[diagnostic_source]` is the YAML diagnostic that does name the file; the
/// nested render, not the wrapper, decides whether the path appears.
///
/// This mirrors [`netsuke::diagnostic_json`]'s rule for the same reason: the
/// JSON path emits `source` and `spans` only for a labelled diagnostic, so
/// both modes agree on when a file is named.
fn names_its_own_source(diagnostic: &dyn Diagnostic) -> bool {
    let names = |candidate: &dyn Diagnostic| {
        candidate.source_code().is_some()
            && candidate
                .labels()
                .is_some_and(|labels| labels.into_iter().next().is_some())
    };
    if names(diagnostic) {
        return true;
    }
    diagnostic
        .diagnostic_source()
        .is_some_and(names_its_own_source)
}

/// Classify a runner failure into the fixed category logged at this boundary.
///
/// A schema rejection is distinguished from every other runner failure because
/// it is the only one an operator can fix by editing the manifest's version
/// declaration rather than its build description. The rendering is unchanged;
/// only the logged category differs, and the diagnostic it accompanies still
/// carries the full explanation.
///
/// The category is decided from the same first `ManifestError` that
/// [`render_failure`] renders, so the log line and the stderr diagnostic
/// cannot describe different failures.
fn failure_kind(err: &Error) -> &'static str {
    let is_schema_rejection = manifest_cause(err).is_some_and(|manifest_error| {
        ManifestFailure::of(manifest_error).is_schema_version_rejection()
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
/// Links are disabled unconditionally, because `miette` emits OSC 8 hyperlinks
/// whenever a diagnostic supplies a [`Diagnostic::url`], and that happens even
/// under a monochrome theme. A `netsuke check` finding always supplies one, so
/// leaving links on would put terminal control sequences into redirected
/// stderr. The URL is still printed as plain text after the code, so nothing is
/// lost from a transcript. This is stricter than the style's `links` flag on
/// purpose: the flag decides whether links are *permitted*, and this boundary
/// is the one place that decides they are not wanted.
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
