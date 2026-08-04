//! Tests for startup diagnostics and the level they are gated by.

use super::*;
use anyhow::{Result, ensure};
use rstest::rstest;
use tracing_subscriber::{fmt, registry::Registry};

/// The level a run starts reporting at, per mode.
///
/// This is the switch that decides whether a locale fallback is ever seen, so
/// each arm is pinned rather than inferred from behaviour elsewhere.
#[rstest]
// Human, not verbose: `WARN`, so a fallback is visible without `--verbose`.
#[case(DiagMode::Human, false, LevelFilter::WARN)]
// Human, verbose: `TRACE`, because config discovery is traced at that level.
#[case(DiagMode::Human, true, LevelFilter::TRACE)]
// JSON silences tracing entirely, whatever the verbosity.
#[case(DiagMode::Json, false, LevelFilter::OFF)]
#[case(DiagMode::Json, true, LevelFilter::OFF)]
fn the_startup_filter_matches_the_mode(
    #[case] mode: DiagMode,
    #[case] verbose: bool,
    #[case] expected: LevelFilter,
) {
    assert_eq!(startup_filter(mode, verbose), expected);
}

/// Run `startup_localizer` for `locale` against a buffering writer, returning
/// what it recorded.
///
/// This is the real startup path — `startup_localizer` calls `build_localizer`
/// — driven through a scoped subscriber so it needs no global state.
fn record_startup(locale: &str) -> (StartupWriter, String) {
    let writer = StartupWriter::buffering();
    let subscriber = Registry::default()
        .with(LevelFilter::WARN)
        .with(fmt::layer().with_writer(writer.clone()).with_ansi(false));
    tracing::subscriber::with_default(subscriber, || {
        drop(cli_localization::build_localizer(Some(locale)));
    });
    let recorded = String::from_utf8_lossy(&writer.buffered()).into_owned();
    (writer, recorded)
}

/// An unsupported startup locale must be recorded before anything is parsed.
///
/// Icelandic ships no catalogue and its language ships none, so the run renders
/// English. The report is buffered at this point, which is what lets it survive
/// until the mode is known without risking a JSON document.
#[test]
fn an_unsupported_startup_locale_is_recorded_before_parsing() -> Result<()> {
    let (_writer, recorded) = record_startup("is-IS");

    ensure!(
        recorded.contains("falling back to the source locale"),
        "the startup path must record the fallback, got {recorded:?}"
    );
    ensure!(
        recorded.contains("is-IS"),
        "the record must name the requested locale, got {recorded:?}"
    );
    Ok(())
}

/// Human mode releases what startup recorded.
#[test]
fn human_mode_releases_the_startup_warning() -> Result<()> {
    let (writer, recorded) = record_startup("is-IS");
    ensure!(!recorded.is_empty(), "expected a recorded warning");

    settle_startup_diagnostics(&writer, DiagMode::Human);

    ensure!(
        writer.buffered().is_empty(),
        "human mode must release the buffer rather than hold it"
    );
    Ok(())
}

/// JSON mode drops it, so stderr carries only the diagnostic document.
#[test]
fn json_mode_discards_the_startup_warning() -> Result<()> {
    let (writer, recorded) = record_startup("is-IS");
    ensure!(!recorded.is_empty(), "expected a recorded warning");

    settle_startup_diagnostics(&writer, DiagMode::Json);

    ensure!(
        writer.buffered().is_empty(),
        "JSON mode must drop the buffer"
    );
    Ok(())
}

/// A supported locale records nothing, or the warning would fire on every run
/// and stop carrying information.
#[test]
fn a_supported_startup_locale_records_nothing() -> Result<()> {
    let (_writer, recorded) = record_startup("fr");
    ensure!(
        recorded.is_empty(),
        "a shipped catalogue must not warn at startup, got {recorded:?}"
    );
    Ok(())
}
