//! Own the binary's diagnostic rendering: mode selection, style, and JSON form.
//!
//! Startup has two diagnostic surfaces and this module keeps them in one place
//! so their shared state cannot drift. The first is Miette's human renderer,
//! whose colour, glyph and link settings come from the resolved display plan.
//! `install_miette_handler` publishes the hook once, at startup, and the hook
//! reads `STYLE` each time a report is rendered rather than capturing it.
//!
//! The ordering that follows is deliberate: `set_style` runs after the
//! configuration merge succeeds, because the resolved display plan does not
//! exist before then. A report created *during* the merge therefore still sees
//! the plain default. That is the intended behaviour — a configuration failure
//! must be readable on a terminal whose capabilities are not yet known — and it
//! is why the default exists at all rather than `set_style` being required
//! before the hook can render anything.
//!
//! The second is `DiagMode`, the choice between human text and versioned JSON
//! documents. `render_runtime_error_json` serves the JSON side and is where the
//! two framings meet: a failure is rendered from the most specific typed error
//! it carries, so callers do not have to decide which one to reach for.

use miette::MietteHandlerOpts;
use miette::Report;
use netsuke::{diagnostic_json, display_plan::DiagnosticStyle, manifest, runner};
use std::sync::OnceLock;

/// Select whether diagnostics render as human text or JSON documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DiagMode {
    /// Human-readable diagnostics written to stderr.
    Human,
    /// Versioned JSON diagnostic documents written to stderr.
    Json,
}

impl DiagMode {
    /// Build the diagnostic mode from a JSON-enabled flag.
    pub(super) const fn from_json_enabled(enabled: bool) -> Self {
        if enabled { Self::Json } else { Self::Human }
    }

    /// Return whether JSON diagnostics are selected.
    pub(super) const fn is_json(self) -> bool {
        matches!(self, Self::Json)
    }
}

/// Plain fallback while startup has not resolved the command's display plan.
const DEFAULT_STYLE: DiagnosticStyle = DiagnosticStyle {
    colour: false,
    unicode: false,
    links: false,
};

/// Store the style selected for the process-wide Miette diagnostic renderer.
static STYLE: OnceLock<DiagnosticStyle> = OnceLock::new();

/// Install Miette's process-wide handler before any diagnostic report exists.
///
/// The handler reads the plan style when each report is created. Until startup
/// resolves that plan, diagnostics use plain ASCII without terminal links.
///
/// # Errors
///
/// Returns an error if another Miette handler has already been installed.
pub(super) fn install_miette_handler() -> Result<(), miette::InstallError> {
    miette::set_hook(Box::new(|_| {
        let style = STYLE.get().copied().unwrap_or(DEFAULT_STYLE);
        Box::new(
            MietteHandlerOpts::new()
                .color(style.colour)
                .unicode(style.unicode)
                .terminal_links(style.links)
                .build(),
        )
    }))
}

/// Store the resolved diagnostic style for reports created later in startup.
pub(super) fn set_style(style: DiagnosticStyle) {
    STYLE.set(style).ok();
}

/// Select the most specific JSON-safe diagnostic representation for a failure.
pub(super) fn render_runtime_error_json(err: &anyhow::Error) -> serde_json::Result<String> {
    if let Some(runner_err) = err.downcast_ref::<runner::RunnerError>() {
        return diagnostic_json::render_diagnostic_json(runner_err);
    }
    if let Some(manifest_err) = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<manifest::ManifestError>())
    {
        return diagnostic_json::render_diagnostic_json(manifest_err);
    }
    if let Some(report) = err.downcast_ref::<Report>() {
        return diagnostic_json::render_report_json(report);
    }
    diagnostic_json::render_error_json(err.as_ref())
}
