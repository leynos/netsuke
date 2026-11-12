//! Tests for human rendering of runner failures.

use super::*;
use crate::test_tracing_capture;
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use miette::{MietteDiagnostic, ThemeCharacters};
use netsuke::localization::{self, keys};
use netsuke::output_prefs;
use tracing_subscriber::filter::LevelFilter;

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
    let rendered = render_failure(&manifest_failure(), &theme_for(theme_prefs()))
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
    let report = failure_report(&manifest_failure(), &theme_for(theme_prefs()))
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
/// `NamedSource` is how a diagnostic says "this is the file and the span I am
/// about", and it is the field [`failure_report`] keys on.
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("unexpected tab indentation")]
#[diagnostic(code(netsuke::manifest::parse))]
struct NamedSourceDiagnostic {
    /// The manifest source text, labelled with the manifest name.
    #[source_code]
    src: miette::NamedSource<String>,
}

/// A diagnostic that names its own source must not repeat the context line.
///
/// Repeating the context line above a diagnostic that already names the path
/// would print it twice, so the rule is keyed on the source, not on the
/// failure's shape. This is the counterpart of
/// [`reports_keep_the_manifest_path_when_the_diagnostic_omits_it`]: together
/// they pin both sides of the decision, so neither branch can be dropped
/// without a failure.
#[test]
fn reports_drop_the_context_line_when_the_diagnostic_names_its_source() -> Result<()> {
    let named = NamedSourceDiagnostic {
        src: miette::NamedSource::new("Netsukefile", String::from("a:\n\tb")),
    };
    let error = manifest::ManifestError::Parse {
        source: Box::new(named),
        message: localization::message(keys::MANIFEST_PARSE),
    };
    let wrapped = anyhow::Error::new(error).context("Failed to load manifest at Netsukefile.");
    let report = failure_report(&wrapped, &theme_for(theme_prefs()))
        .context("a named source should report")?;
    let occurrences = report.matches("Netsukefile").count();
    ensure!(
        occurrences == 1,
        "a diagnostic naming its source must not repeat the path: {report}"
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
    let rendered = render_failure(&error, &theme_for(theme_prefs()))
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
        render_failure(&error, &theme_for(theme_prefs())).is_none(),
        "a failure carrying no diagnostic must not gain a second rendering"
    );
    Ok(())
}

#[test]
fn emoji_preferences_select_the_unicode_theme() -> Result<()> {
    // `resolve_with` takes the `--no-emoji` value, so `Some(false)` allows
    // emoji and `Some(true)` suppresses them.
    let emoji_allowed = theme_for(theme_prefs());
    let emoji_suppressed = theme_for(output_prefs::resolve_with(Some(true), |_| None));
    // `GraphicalTheme` derives only `Debug` and `Clone`, so the direction is
    // asserted on the glyph set, which does derive `PartialEq`.
    ensure!(
        emoji_allowed.characters == ThemeCharacters::unicode(),
        "allowing emoji must select the Unicode glyph set: {emoji_allowed:?}"
    );
    ensure!(
        emoji_suppressed.characters == ThemeCharacters::ascii(),
        "suppressing emoji must select the ASCII glyph set: {emoji_suppressed:?}"
    );
    ensure!(
        emoji_allowed.characters.error != emoji_suppressed.characters.error,
        "the two preferences must not resolve to the same glyph set"
    );
    Ok(())
}

/// Resolve preferences that allow emoji without consulting the environment.
fn theme_prefs() -> OutputPrefs {
    output_prefs::resolve_with(Some(false), |_| None)
}

/// A schema rejection is logged under its own category, not as a plain runner
/// failure, so an operator can count version faults apart from build faults.
#[test]
fn schema_rejections_log_their_own_bounded_error_category() -> Result<()> {
    let prefs = theme_prefs();
    let (exit_code, events) =
        test_tracing_capture::with_test_subscriber(LevelFilter::TRACE, |captured| {
            let exit_code = handle_failure(&manifest_failure(), &prefs);
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
    let prefs = theme_prefs();
    let (exit_code, events) =
        test_tracing_capture::with_test_subscriber(LevelFilter::TRACE, |captured| {
            let exit_code = handle_failure(&other, &prefs);
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
    let prefs = theme_prefs();
    let (exit_code, events) =
        test_tracing_capture::with_test_subscriber(LevelFilter::TRACE, |captured| {
            let exit_code = handle_failure(&cycle_error(PRIVATE_TARGET), &prefs);
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
