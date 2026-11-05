//! Tests for human rendering of runner failures.

use super::*;
use crate::test_tracing_capture;
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use miette::MietteDiagnostic;
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
    let inner = MietteDiagnostic::new(
        "[netsuke::manifest::schema_version] declares manifest schema 1.1, \
         but this Netsuke supports schema 1.0.",
    );
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
    let informational = theme_for(theme_prefs());
    let plain = theme_for(output_prefs::resolve_with(Some(true), |_| None));
    ensure!(
        informational.characters.error != plain.characters.error,
        "the emoji-allowed theme must draw different glyphs from the plain one"
    );
    Ok(())
}

/// Resolve preferences without consulting the process environment.
fn theme_prefs() -> OutputPrefs {
    output_prefs::resolve_with(Some(false), |_| None)
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
