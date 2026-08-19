//! Tests for startup diagnostics and the level they are gated by.

use super::*;
use crate::config_resolution::config_err_to_exit;
use anyhow::{Result, ensure};
use netsuke::localization::keys;
use ortho_config::OrthoError;
use rstest::rstest;
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
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

impl locale_resolution::LocaleEnvProvider for EmptyEnv {
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
fn record_startup(locale: &str) -> Result<(StartupWriter, String)> {
    let args: Vec<OsString> = ["netsuke", "--locale", locale]
        .into_iter()
        .map(OsString::from)
        .collect();
    record_startup_with(&args, &EmptyEnv)
}

/// Run the startup orchestration over `args` and `env`, recording what it says.
///
/// The general form behind [`record_startup`], for the tests that need the
/// locale to arrive by a route other than `--locale`. Both share the lock and
/// the restoration, which is the part that must not be reimplemented per test.
fn record_startup_with<E: locale_resolution::LocaleEnvProvider>(
    args: &[OsString],
    env: &E,
) -> Result<(StartupWriter, String)> {
    // `startup_localizer` writes the process-global localizer, so the shared
    // lock is held across installation and restoration. Without it another test
    // doing the same could capture this one's override as its "previous" and
    // later restore the wrong value — the lock only serializes the tests that
    // take it.
    let _lock = test_support::localizer_test_lock()
        .map_err(|error| anyhow::anyhow!("localizer test lock poisoned: {error}"))?;
    let writer = StartupWriter::buffering();
    let subscriber = Registry::default()
        .with(LevelFilter::WARN)
        .with(fmt::layer().with_writer(writer.clone()).with_ansi(false));

    let previous = localization::localizer();
    tracing::subscriber::with_default(subscriber, || {
        drop(startup_localizer(args, env, &NoSystemLocale));
    });
    localization::set_localizer(previous);

    let recorded = String::from_utf8_lossy(&writer.buffered()).into_owned();
    Ok((writer, recorded))
}

/// An unsupported startup locale must be buffered by the startup orchestration.
///
/// Icelandic ships no catalogue and its language ships none, so the run renders
/// English. The report is held in the writer at this point — not yet on stderr
/// — which is what lets it survive until the mode is known without risking a
/// JSON document.
#[test]
fn an_unsupported_startup_locale_is_recorded_before_parsing() -> Result<()> {
    let (_writer, recorded) = record_startup("is-IS")?;

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

impl locale_resolution::LocaleEnvProvider for EnvWithLocale {
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
    let args = vec![OsString::from("netsuke")];
    let (_writer, recorded) = record_startup_with(&args, &EnvWithLocale("is-IS"))?;

    ensure!(
        recorded.contains("is-IS"),
        "the environment locale must reach the localizer, got {recorded:?}"
    );
    Ok(())
}

/// Settlement empties the buffer, whichever way the mode sends it.
///
/// The two modes differ in *where* the recorded warning goes — released to
/// stderr, or dropped — but both must leave the writer holding nothing, so the
/// startup buffer never leaks into the rest of the run.
#[rstest]
#[case(DiagMode::Human, "human mode must release the buffer to stderr")]
#[case(DiagMode::Json, "JSON mode must drop the buffer")]
fn settling_empties_the_startup_buffer(
    #[case] mode: DiagMode,
    #[case] expectation: &str,
) -> Result<()> {
    let (writer, recorded) = record_startup("is-IS")?;
    ensure!(
        !recorded.is_empty(),
        "expected startup to record a warning before settlement"
    );

    settle_startup_diagnostics(&writer, mode);

    ensure!(writer.buffered().is_empty(), "{expectation}");
    Ok(())
}

/// A supported locale records nothing, or the warning would fire on every run
/// and stop carrying information.
#[test]
fn a_supported_startup_locale_records_nothing() -> Result<()> {
    let (_writer, recorded) = record_startup("fr")?;
    ensure!(
        recorded.is_empty(),
        "a shipped catalogue must not warn at startup, got {recorded:?}"
    );
    Ok(())
}

/// The startup orchestration installs the resolved localizer globally, and the
/// previous one is restored once the scope ends.
///
/// `startup_localizer` mutates process-global state, so this holds the shared
/// test-localizer lock across installation, observation, and restoration. A
/// second thread emits one controlled event while the startup localizer is
/// installed, coordinated by barriers rather than timing, to show that the
/// buffered writer is shared correctly and that a concurrent emitter cannot
/// observe a half-installed state.
#[test]
fn startup_installs_and_restores_the_global_localizer() -> Result<()> {
    let _lock = test_support::localizer_test_lock()
        .map_err(|error| anyhow::anyhow!("localizer test lock poisoned: {error}"))?;

    let before = localization::message(keys::CLI_ABOUT).to_string();
    let writer = StartupWriter::buffering();
    let subscriber = Registry::default()
        .with(LevelFilter::WARN)
        .with(fmt::layer().with_writer(writer.clone()).with_ansi(false));

    let args: Vec<OsString> = ["netsuke", "--locale", "fr"]
        .into_iter()
        .map(OsString::from)
        .collect();
    // A guard, not a manual restore at the end: every `?` and `ensure!` below
    // is an early exit, and a manual restore after them would be skipped on
    // any of those paths, leaving the French localizer installed for whichever
    // test runs next. Installing the current localizer over itself is a no-op
    // that captures it as the guard's restore target.
    let previous = localization::localizer();
    let restore = localization::set_localizer_for_tests(Arc::clone(&previous));

    // Two rendezvous: the first once the localizer is installed, the second
    // once the concurrent event has been emitted.
    let installed = Arc::new(Barrier::new(2));
    let emitted = Arc::new(Barrier::new(2));
    let during = Arc::new(Mutex::new(String::new()));

    let (thread_installed, thread_emitted, thread_during, thread_writer) = (
        Arc::clone(&installed),
        Arc::clone(&emitted),
        Arc::clone(&during),
        writer.clone(),
    );
    let observer = thread::spawn(move || {
        thread_installed.wait();
        // Observed while the startup localizer is installed.
        let rendered = localization::message(keys::CLI_ABOUT).to_string();
        if let Ok(mut slot) = thread_during.lock() {
            *slot = rendered;
        }
        // `with_default` installs a *thread-local* subscriber, so an event
        // emitted here would reach the main thread's subscriber only if this
        // thread had one of its own. Installing one over a clone of the same
        // writer is what makes this exercise the shared buffer rather than
        // silently emit into nothing.
        let observer_subscriber = Registry::default()
            .with(LevelFilter::WARN)
            .with(fmt::layer().with_writer(thread_writer).with_ansi(false));
        tracing::subscriber::with_default(observer_subscriber, || {
            tracing::warn!(target: "concurrent", "observed during startup");
        });
        thread_emitted.wait();
    });

    tracing::subscriber::with_default(subscriber, || {
        drop(startup_localizer(&args, &EmptyEnv, &NoSystemLocale));
        installed.wait();
        emitted.wait();
    });

    observer
        .join()
        .map_err(|_| anyhow::anyhow!("observer thread panicked"))?;

    let rendered_during_startup = during
        .lock()
        .map_err(|error| anyhow::anyhow!("observation lock poisoned: {error}"))?
        .clone();
    ensure!(
        rendered_during_startup != before,
        "the concurrent observer must see the installed French localizer, \
         got {rendered_during_startup:?}"
    );

    let buffered = String::from_utf8_lossy(&writer.buffered()).into_owned();
    ensure!(
        buffered.contains("observed during startup"),
        "the concurrent thread's event must reach the shared writer, got {buffered:?}"
    );

    // Dropped explicitly so the final assertion observes the restored state;
    // the guard would otherwise restore only after the assertion ran.
    drop(restore);
    let after = localization::message(keys::CLI_ABOUT).to_string();
    ensure!(
        after == before,
        "the previous localizer must be restored, got {after:?} rather than {before:?}"
    );
    Ok(())
}

/// Human-readable config failures identify their bounded operation and category.
#[test]
fn config_load_errors_include_operational_context() -> Result<()> {
    let writer = StartupWriter::buffering();
    let subscriber = Registry::default()
        .with(LevelFilter::ERROR)
        .with(fmt::layer().with_writer(writer.clone()).with_ansi(false));
    let validation_error = OrthoError::Validation {
        key: "jobs".into(),
        message: "must be positive".into(),
    };
    let file_error = OrthoError::File {
        path: "private.toml".into(),
        source: Box::new(std::io::Error::other("read failure")),
    };

    tracing::subscriber::with_default(subscriber, || -> Result<()> {
        ensure!(
            config_err_to_exit(
                &validation_error,
                DiagMode::Human,
                observability::DIAG_MODE_OPERATION,
            ) == ExitCode::FAILURE,
            "a diagnostic-mode config error must fail the command"
        );
        ensure!(
            config_err_to_exit(&file_error, DiagMode::Human, observability::MERGE_OPERATION)
                == ExitCode::FAILURE,
            "a merge config error must fail the command"
        );
        Ok(())
    })?;

    let recorded = String::from_utf8_lossy(&writer.buffered()).into_owned();
    ensure!(
        recorded.contains("operation=\"diag_mode_resolution\"")
            && recorded.contains("error_category=\"validation\""),
        "diagnostic-mode failures must identify their operation and category: {recorded:?}"
    );
    ensure!(
        recorded.contains("operation=\"config_merge\"")
            && recorded.contains("error_category=\"io\""),
        "merge failures must identify their operation and category: {recorded:?}"
    );
    ensure!(
        !recorded.contains("private.toml") && !recorded.contains("read failure"),
        "human-readable config errors must not expose error details: {recorded:?}"
    );
    Ok(())
}
