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
///
/// Startup calls this exactly once, after the configuration merge succeeds, so
/// a rejected write means a caller installed a style twice. That is a defect in
/// the caller rather than a condition to recover from, but it must not pass
/// silently: the rejected style is returned so the call site can report it,
/// and rendering continues with whichever style was installed first.
///
/// # Errors
///
/// Returns the style that was *not* stored when one was already installed.
pub(super) fn set_style(style: DiagnosticStyle) -> Result<(), DiagnosticStyle> {
    store_style(&STYLE, style)
}

/// Store `style` in `slot`, returning the style the slot rejected.
///
/// The slot is a parameter rather than captured from [`STYLE`] so the one-shot
/// contract can be exercised directly. A test that installed into the real slot
/// would decide the outcome for every later test sharing the process, and would
/// have nothing to assert about the first install.
pub(super) fn store_style(
    slot: &OnceLock<DiagnosticStyle>,
    style: DiagnosticStyle,
) -> Result<(), DiagnosticStyle> {
    slot.set(style)
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

#[cfg(test)]
mod tests {
    //! Pin the one-shot installation contract of [`super::store_style`].
    use super::{DiagnosticStyle, OnceLock, store_style};

    /// The first installed diagnostic style wins and the second is handed back.
    ///
    /// `set_style` runs once per process, after the configuration merge. The
    /// contract worth pinning is what happens when that assumption is broken:
    /// the earlier style must survive, because reports already rendered under it
    /// cannot be recalled, and the refused style must come back so the call site
    /// can say so rather than losing the discrepancy.
    ///
    /// The slot is local rather than the process-wide `STYLE`: installing into
    /// the real one would decide every later test's rendering in this process,
    /// and there would be nothing left to assert about the first install.
    #[test]
    fn only_the_first_style_is_installed() {
        let first = DiagnosticStyle {
            colour: true,
            unicode: true,
            links: true,
        };
        let second = DiagnosticStyle {
            colour: false,
            unicode: false,
            links: false,
        };
        let slot = OnceLock::new();

        assert_eq!(
            store_style(&slot, first),
            Ok(()),
            "the first install succeeds"
        );
        assert_eq!(
            store_style(&slot, second),
            Err(second),
            "a second install is refused and returns the style it refused"
        );
        assert_eq!(
            slot.get().copied(),
            Some(first),
            "rendering keeps the first style"
        );
    }
}
