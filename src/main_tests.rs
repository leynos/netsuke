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

/// An environment that reports nothing, so `--locale` decides the outcome.
struct EmptyEnv;

impl locale_resolution::EnvProvider for EmptyEnv {
    fn var(&self, _key: &str) -> Option<String> {
        None
    }
}

/// A system locale provider that reports nothing, for the same reason.
struct NoSystemLocale;

impl locale_resolution::SystemLocale for NoSystemLocale {
    fn system_locale(&self) -> Option<String> {
        None
    }
}

/// Drive the real startup orchestration for `locale`, returning the writer and
/// what it buffered.
///
/// This calls `startup_localizer` — the function `run_with_args` calls — rather
/// than reaching past it to `build_localizer`, so locale resolution and the
/// installed writer are both exercised. The environment and system locale are
/// injected as empty, so the outcome depends only on the `--locale` argument
/// and no process state is read.
///
/// `run_with_args` itself is not called: it parses the command line, and clap
/// terminates the process on help, version, and usage errors, which a unit test
/// cannot survive. `tests/startup_diagnostics_tests.rs` covers those paths by
/// running the built binary.
///
/// `startup_localizer` installs a process-global localizer, so the previous one
/// is restored before returning.
fn record_startup(locale: &str) -> (StartupWriter, String) {
    let writer = StartupWriter::buffering();
    let subscriber = Registry::default()
        .with(LevelFilter::WARN)
        .with(fmt::layer().with_writer(writer.clone()).with_ansi(false));

    let args: Vec<OsString> = ["netsuke", "--locale", locale]
        .into_iter()
        .map(OsString::from)
        .collect();
    let previous = localization::localizer();
    tracing::subscriber::with_default(subscriber, || {
        drop(startup_localizer(&args, &EmptyEnv, &NoSystemLocale));
    });
    localization::set_localizer(previous);

    let recorded = String::from_utf8_lossy(&writer.buffered()).into_owned();
    (writer, recorded)
}

/// An unsupported startup locale must be buffered by the startup orchestration.
///
/// Icelandic ships no catalogue and its language ships none, so the run renders
/// English. The report is held in the writer at this point — not yet on stderr
/// — which is what lets it survive until the mode is known without risking a
/// JSON document.
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

/// An environment reporting `NETSUKE_LOCALE`.
struct EnvWithLocale(&'static str);

impl locale_resolution::EnvProvider for EnvWithLocale {
    fn var(&self, key: &str) -> Option<String> {
        (key == "NETSUKE_LOCALE").then(|| self.0.to_owned())
    }
}

/// The orchestration resolves the locale rather than being handed one.
///
/// With no `--locale` argument the tag can only reach `build_localizer` through
/// `resolve_startup_locale` consulting the injected environment. A test that
/// called `build_localizer` directly would pass whatever happened here, so this
/// is what distinguishes exercising the startup path from bypassing it.
#[test]
fn the_startup_path_resolves_the_locale_from_the_environment() -> Result<()> {
    let writer = StartupWriter::buffering();
    let subscriber = Registry::default()
        .with(LevelFilter::WARN)
        .with(fmt::layer().with_writer(writer.clone()).with_ansi(false));

    let args = vec![OsString::from("netsuke")];
    let previous = localization::localizer();
    tracing::subscriber::with_default(subscriber, || {
        drop(startup_localizer(
            &args,
            &EnvWithLocale("is-IS"),
            &NoSystemLocale,
        ));
    });
    localization::set_localizer(previous);

    let recorded = String::from_utf8_lossy(&writer.buffered()).into_owned();
    ensure!(
        recorded.contains("is-IS"),
        "the environment locale must reach the localizer, got {recorded:?}"
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
