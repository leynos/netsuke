//! Tests for human rendering of runner failures.

use super::*;
use crate::test_tracing_capture;
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use miette::{MietteDiagnostic, ThemeCharacters};
use netsuke::localization::{self, keys};
use netsuke::output_prefs;
use strip_ansi_escapes::strip;
use tracing_subscriber::filter::LevelFilter;

mod runner_diagnostics;

/// Build the circular-dependency error the runner reports, keeping the target
/// name private so its absence from tracing output is meaningful.
fn cycle_error(private_target: &str) -> anyhow::Error {
    let cycle = vec![
        Utf8PathBuf::from(private_target),
        Utf8PathBuf::from(private_target),
    ];
    let message =
        localization::message(keys::IR_CIRCULAR_DEPENDENCY).with_arg("cycle", format!("{cycle:?}"));
    anyhow::Error::new(IrGenError::CircularDependency {
        cycle,
        missing_dependencies: Vec::new(),
        message,
    })
}

/// Build a manifest failure wrapped in the same contextual layer the runner
/// adds, so the test exercises the chain rather than a bare diagnostic.
fn manifest_failure() -> anyhow::Error {
    // The code is set as a field, not baked into the message text, so a
    // renderer that drops the nested code cannot pass on the text alone.
    let inner = MietteDiagnostic::new(
        "declares manifest schema 1.1, but this Netsuke supports schema 1.0.",
    )
    .with_code("netsuke::manifest::schema_version");
    let error = manifest::ManifestError::Parse {
        source: Box::new(inner),
        message: localization::message(keys::MANIFEST_PARSE),
    };
    anyhow::Error::new(error).context("Failed to load manifest at Netsukefile.")
}

#[test]
fn manifest_failures_render_their_inner_diagnostic() -> Result<()> {
    let rendered = render_failure(
        &manifest_failure(),
        &theme_for(presentation_style(false, true)),
    )
    .context("a manifest failure should render a diagnostic")?;
    ensure!(
        rendered.contains("netsuke::manifest::schema_version"),
        "the rendered diagnostic must carry the rejection's stable code: {rendered}"
    );
    ensure!(
        rendered.contains("this Netsuke supports schema 1.0"),
        "the rendered diagnostic must carry the actionable message: {rendered}"
    );
    Ok(())
}

/// A schema rejection must still say which manifest was refused.
///
/// `SchemaVersionDiagnostic` carries no source code, so its rendering names
/// the declared and supported schema but never the file. The context line is
/// the only place the path appears, which is why it is kept for a diagnostic
/// that does not name its artefact. Rendering the diagnostic alone would make
/// a refusal unattributable in a project with several manifests.
#[test]
fn reports_keep_the_manifest_path_when_the_diagnostic_omits_it() -> Result<()> {
    let report = failure_report(
        &manifest_failure(),
        &theme_for(presentation_style(false, true)),
    )
    .context("a manifest failure should render a report")?;
    ensure!(
        report.contains("Failed to load manifest at Netsukefile."),
        "the report must name the manifest the context line identified: {report}"
    );
    ensure!(
        report.contains("netsuke::manifest::schema_version"),
        "the report must still carry the rejection's stable code: {report}"
    );
    Ok(())
}

/// A diagnostic that names the file it is about, as a YAML failure does.
///
/// `MietteDiagnostic` cannot carry a source, so the shape is built here: a
/// `NamedSource` plus a `#[label]` is how a diagnostic says "this is the file,
/// and this is the span I am about". The label is not decoration. `miette`
/// returns before drawing a snippet when a diagnostic has no labels, so without
/// one the file name is never printed however good the source is — and a test
/// built on the source alone would pass while proving nothing.
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("unexpected tab indentation")]
#[diagnostic(code(netsuke::manifest::parse))]
struct NamedSourceDiagnostic {
    /// The manifest source text, labelled with the manifest name.
    #[source_code]
    src: miette::NamedSource<String>,
    /// The tab the parser rejected.
    #[label("unexpected tab")]
    span: miette::SourceSpan,
}

/// A diagnostic that names its own source must not repeat the context line.
///
/// The assertion is on the *context line*, not on how many times the path
/// appears. A snippet names the file and so does the context line, so a count
/// would be satisfied by a snippet with the context line still above it — the
/// duplicate this rule exists to prevent. Requiring its absence makes the test
/// fail if the decision is ever taken on the wrong signal.
///
/// The counterpart of
/// [`reports_keep_the_manifest_path_when_the_diagnostic_omits_it`]: together
/// they pin both sides, so neither branch can be dropped without a failure.
#[test]
fn reports_drop_the_context_line_when_the_diagnostic_names_its_source() -> Result<()> {
    let named = NamedSourceDiagnostic {
        src: miette::NamedSource::new("Netsukefile", String::from("a:\n\tb")),
        span: miette::SourceSpan::from((3, 1)),
    };
    let error = manifest::ManifestError::Parse {
        source: Box::new(named),
        message: localization::message(keys::MANIFEST_PARSE),
    };
    let wrapped = anyhow::Error::new(error).context("Failed to load manifest at Netsukefile.");
    let report = failure_report(&wrapped, &theme_for(presentation_style(false, true)))
        .context("a named source should report")?;
    ensure!(
        report.contains("Netsukefile"),
        "the report must name the file the diagnostic is about: {report}"
    );
    ensure!(
        !report.contains("Failed to load manifest at Netsukefile."),
        "a diagnostic naming its source must not repeat the context line: {report}"
    );
    Ok(())
}

/// A source without a label names nothing, so the context line must stay.
///
/// This is the case the rule exists for: `miette` draws no snippet for an
/// unlabelled diagnostic, so the source it carries never reaches the report.
/// Deciding on the source alone would drop the context line here and leave the
/// failure naming no file at all.
#[test]
fn reports_keep_the_context_line_when_the_source_carries_no_label() -> Result<()> {
    /// A source with no label, so no snippet is drawn from it.
    #[derive(Debug, thiserror::Error, miette::Diagnostic)]
    #[error("unexpected tab indentation")]
    #[diagnostic(code(netsuke::manifest::parse))]
    struct UnlabelledSourceDiagnostic {
        /// Manifest source text that nothing points into.
        #[source_code]
        src: miette::NamedSource<String>,
    }

    let unlabelled = UnlabelledSourceDiagnostic {
        src: miette::NamedSource::new("Netsukefile", String::from("a:\n\tb")),
    };
    let error = manifest::ManifestError::Parse {
        source: Box::new(unlabelled),
        message: localization::message(keys::MANIFEST_PARSE),
    };
    let wrapped = anyhow::Error::new(error).context("Failed to load manifest at Netsukefile.");
    let report = failure_report(&wrapped, &theme_for(presentation_style(false, true)))
        .context("an unlabelled source reports")?;
    ensure!(
        report.contains("Failed to load manifest at Netsukefile."),
        "without a snippet the context line is the only place the path appears: {report}"
    );
    Ok(())
}

/// A diagnostic that supplies a URL, as every `netsuke check` finding does.
fn linked_diagnostic() -> MietteDiagnostic {
    MietteDiagnostic::new("a finding with a rule reference")
        .with_code("netsuke::lint::example")
        .with_url("https://example.invalid/rule")
}

#[test]
fn rendered_diagnostics_emit_no_terminal_hyperlinks() -> Result<()> {
    let error = anyhow::Error::new(manifest::ManifestError::Parse {
        source: Box::new(linked_diagnostic()),
        message: localization::message(keys::MANIFEST_PARSE),
    });
    let rendered = render_failure(&error, &theme_for(presentation_style(false, true)))
        .context("a diagnostic carrying a URL should still render")?;
    ensure!(
        !rendered.contains('\u{1b}'),
        "plain diagnostic output must contain no escape sequences: {rendered:?}"
    );
    ensure!(
        rendered.contains("https://example.invalid/rule"),
        "the rule reference must survive as plain text: {rendered}"
    );
    Ok(())
}

#[test]
fn failures_without_a_diagnostic_render_nothing_extra() -> Result<()> {
    let error = anyhow::Error::msg("plain failure");
    ensure!(
        render_failure(&error, &theme_for(presentation_style(false, true))).is_none(),
        "a failure carrying no diagnostic must not gain a second rendering"
    );
    Ok(())
}

#[test]
fn style_flags_select_the_unicode_and_ascii_glyph_sets() -> Result<()> {
    let unicode = theme_for(presentation_style(true, true));
    let ascii = theme_for(presentation_style(true, false));
    // `GraphicalTheme` derives only `Debug` and `Clone`, so the direction is
    // asserted on the glyph set, which does derive `PartialEq`.
    ensure!(
        unicode.characters == ThemeCharacters::unicode(),
        "a Unicode style must select the Unicode glyph set: {unicode:?}"
    );
    ensure!(
        ascii.characters == ThemeCharacters::ascii(),
        "an ASCII style must select the ASCII glyph set: {ascii:?}"
    );
    ensure!(
        unicode.characters.error != ascii.characters.error,
        "the two styles must not resolve to the same glyph set"
    );
    Ok(())
}

/// Colour is a property of the style, not of the glyph set.
///
/// The two decisions are independent: four combinations are reachable, and a
/// theme chosen from the glyph flag alone would silently drop colour from a
/// stream the plan had already decided to style. That is why the theme is
/// selected on the pair rather than on `unicode` alone.
///
/// `miette`'s `ThemeStyles` implements neither `PartialEq` nor anything else
/// that distinguishes two styles, so this asserts on rendered output: a
/// diagnostic under a colour-on style must carry SGR, and the same diagnostic
/// under a colour-off style must not, while both keep the same glyphs.
#[test]
fn style_colour_is_independent_of_the_glyph_set() -> Result<()> {
    let diagnostic = MietteDiagnostic::new("the manifest was refused")
        .with_code("netsuke::manifest::schema_version");
    let coloured = render_diagnostic(&diagnostic, &theme_for(presentation_style(true, true)));
    let plain = render_diagnostic(&diagnostic, &theme_for(presentation_style(false, true)));
    ensure!(
        coloured.contains('\u{1b}'),
        "a colour-on style must emit ANSI escapes: {coloured:?}"
    );
    ensure!(
        !plain.contains('\u{1b}'),
        "a colour-off style must emit no ANSI escapes: {plain:?}"
    );
    ensure!(
        strip(coloured.as_bytes()) == plain.as_bytes(),
        "colour must be the only difference between the two renderings"
    );
    Ok(())
}

/// Build a diagnostic style carrying the given colour and glyph decisions.
fn presentation_style(colour: bool, unicode: bool) -> DiagnosticStyle {
    DiagnosticStyle {
        colour,
        unicode,
        links: false,
    }
}

/// Resolve preferences that allow emoji without consulting the environment.
fn theme_prefs() -> OutputPrefs {
    output_prefs::resolve_with(Some(false), |_| None)
}

/// Build the presentation a failure is rendered under, without ambient state.
fn presentation() -> Presentation {
    Presentation {
        prefs: theme_prefs(),
        style: presentation_style(false, true),
    }
}

/// A schema rejection is logged under its own category, not as a plain runner
/// failure, so an operator can count version faults apart from build faults.
#[test]
fn schema_rejections_log_their_own_bounded_error_category() -> Result<()> {
    let presentation = presentation();
    let (exit_code, events) =
        test_tracing_capture::with_test_subscriber(LevelFilter::TRACE, |captured| {
            let exit_code = handle_failure(&manifest_failure(), presentation);
            (exit_code, captured.snapshot())
        });

    ensure!(
        exit_code == ExitCode::FAILURE,
        "a schema rejection must fail the command"
    );
    let event = events
        .iter()
        .find(|event| event.contains("runner failed"))
        .ok_or_else(|| anyhow::anyhow!("expected a runner failure event, got {events:?}"))?;
    ensure!(
        event.contains("error_kind=\"manifest_schema_version\""),
        "a schema rejection must not be logged as a plain runner failure: {event}"
    );
    Ok(())
}

/// Any other manifest failure keeps the general runner category.
#[test]
fn plain_manifest_rejections_keep_the_general_error_category() -> Result<()> {
    let other = anyhow::Error::new(manifest::ManifestError::Parse {
        source: Box::new(MietteDiagnostic::new("the manifest is structurally wrong")),
        message: localization::message(keys::MANIFEST_PARSE),
    });
    let presentation = presentation();
    let (exit_code, events) =
        test_tracing_capture::with_test_subscriber(LevelFilter::TRACE, |captured| {
            let exit_code = handle_failure(&other, presentation);
            (exit_code, captured.snapshot())
        });

    ensure!(
        exit_code == ExitCode::FAILURE,
        "the failure must be reported"
    );
    let event = events
        .iter()
        .find(|event| event.contains("runner failed"))
        .ok_or_else(|| anyhow::anyhow!("expected a runner failure event, got {events:?}"))?;
    ensure!(
        event.contains("error_kind=\"runner\""),
        "a non-schema manifest failure keeps the general category: {event}"
    );
    Ok(())
}

/// Keep manifest-derived cycle paths out of runner tracing events.
#[test]
fn human_cycle_failures_log_only_the_bounded_error_category() -> Result<()> {
    const PRIVATE_TARGET: &str = "manifest-private-target";
    let presentation = presentation();
    let (exit_code, events) =
        test_tracing_capture::with_test_subscriber(LevelFilter::TRACE, |captured| {
            let exit_code = handle_failure(&cycle_error(PRIVATE_TARGET), presentation);
            (exit_code, captured.snapshot())
        });

    ensure!(
        exit_code == ExitCode::FAILURE,
        "a cycle diagnostic must fail the command"
    );
    let event = events
        .iter()
        .find(|event| event.contains("runner failed"))
        .ok_or_else(|| anyhow::anyhow!("expected a runner failure event, got {events:?}"))?;
    ensure!(
        event.contains("error_kind=\"circular_dependency\""),
        "the event must use the fixed error category: {event}"
    );
    ensure!(
        !event.contains(PRIVATE_TARGET),
        "the event must not expose manifest target names: {event}"
    );
    Ok(())
}
