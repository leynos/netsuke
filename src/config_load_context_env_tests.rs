//! Tests for configuration-load environment injection.
//!
//! These cases prove the startup context supplies the same in-memory
//! configuration environment to early JSON resolution and the cached merge.

use super::super::super::{DiagMode, StartupWriter, cli};
use super::super::{ConfigurationLoadContext, resolve_configuration};
use super::{ConfigurationLoadScenario, configuration_clock};
use anyhow::{Result, ensure};
use clap::CommandFactory;
use std::cell::Cell;
use std::ffi::OsString;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use tempfile::tempdir;
use tracing::{Subscriber, field::Visit, span::Id};
use tracing_subscriber::{
    Layer, filter::LevelFilter, layer::Context as LayerContext, prelude::*, registry::LookupSpan,
};

/// Captures fields recorded on the retained discovery span.
#[derive(Clone, Default)]
struct DiscoverySpanCapture {
    fields: Arc<Mutex<Vec<String>>>,
}

impl DiscoverySpanCapture {
    /// Return every field recorded on the discovery span.
    fn fields(&self) -> Vec<String> {
        self.fields
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl<S> Layer<S> for DiscoverySpanCapture
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_record(&self, id: &Id, values: &tracing::span::Record<'_>, ctx: LayerContext<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        if span.metadata().name() != "collect_diag_file_layers" {
            return;
        }
        let mut visitor = SpanFieldVisitor::default();
        values.record(&mut visitor);
        self.fields
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend(visitor.0);
    }
}

/// Renders recorded span fields using the same stable representation as tracing capture.
#[derive(Default)]
struct SpanFieldVisitor(Vec<String>);

impl Visit for SpanFieldVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.push(format!("{}={value:?}", field.name()));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.push(format!("{}={value:?}", field.name()));
    }
}

/// In-memory configuration environment for startup-metrics test scenarios.
pub(super) struct EmptyConfigEnv;

impl cli::ConfigEnvProvider for EmptyConfigEnv {
    fn get(&self, _key: &str) -> Option<OsString> {
        None
    }

    fn entries(&self) -> Vec<(OsString, OsString)> {
        Vec::new()
    }
}

/// In-memory provider that records which configuration phase reads it.
struct RecordingConfigEnv {
    json_reads: Cell<usize>,
    entries_reads: Cell<usize>,
}

impl RecordingConfigEnv {
    /// Construct a provider whose JSON lookup and merge snapshot differ.
    const fn new() -> Self {
        Self {
            json_reads: Cell::new(0),
            entries_reads: Cell::new(0),
        }
    }
}

impl cli::ConfigEnvProvider for RecordingConfigEnv {
    fn get(&self, key: &str) -> Option<OsString> {
        if key == "NETSUKE_JSON" {
            self.json_reads.set(self.json_reads.get() + 1);
            Some(OsString::from("true"))
        } else {
            None
        }
    }

    fn entries(&self) -> Vec<(OsString, OsString)> {
        self.entries_reads.set(self.entries_reads.get() + 1);
        vec![(OsString::from("NETSUKE_JOBS"), OsString::from("7"))]
    }
}

/// The context forwards one injected provider to both configuration phases.
#[test]
fn configuration_context_uses_its_injected_environment_for_both_phases() -> Result<()> {
    let parsed_cli = cli::Cli::default();
    let matches = cli::Cli::command().get_matches_from(["netsuke"]);
    let startup_writer = StartupWriter::buffering();
    let config_env = RecordingConfigEnv::new();
    let context = ConfigurationLoadContext {
        parsed_cli: &parsed_cli,
        matches: &matches,
        startup_mode: DiagMode::Human,
        startup_writer: &startup_writer,
        config_env: &config_env,
    };
    let clock = configuration_clock(
        ConfigurationLoadScenario::SuccessfulMerge,
        Duration::from_millis(1),
    )?;
    let capture = DiscoverySpanCapture::default();
    let subscriber =
        tracing_subscriber::registry().with(capture.clone().with_filter(LevelFilter::TRACE));

    let merged =
        tracing::subscriber::with_default(subscriber, || resolve_configuration(&context, &clock))
            .map_err(|code| anyhow::anyhow!("configuration should succeed, got {code:?}"))?;

    ensure!(
        config_env.json_reads.get() == 1,
        "early JSON resolution should read NETSUKE_JSON through the context provider"
    );
    ensure!(
        config_env.entries_reads.get() == 1,
        "cached merge should read configuration entries through the context provider"
    );
    ensure!(
        merged.jobs == Some(7),
        "cached merge should apply the injected NETSUKE_JOBS value"
    );
    let fields = capture.fields();
    ensure!(
        fields.contains(&"outcome=\"success\"".to_owned()),
        "startup must record a successful discovery outcome: {fields:?}"
    );
    ensure!(
        !fields
            .iter()
            .any(|field| field.starts_with("error_category=")),
        "successful discovery must not record an error category: {fields:?}"
    );
    Ok(())
}

/// Startup records the retained discovery outcome on the documented trace span.
#[test]
fn startup_resolution_records_the_retained_discovery_span() -> Result<()> {
    let temp = tempdir()?;
    let parsed_cli = cli::Cli {
        config: Some(temp.path().join("missing.toml")),
        ..cli::Cli::default()
    };
    let matches = cli::Cli::command().get_matches_from(["netsuke"]);
    let startup_writer = StartupWriter::buffering();
    let context = ConfigurationLoadContext {
        parsed_cli: &parsed_cli,
        matches: &matches,
        startup_mode: DiagMode::Human,
        startup_writer: &startup_writer,
        config_env: &EmptyConfigEnv,
    };
    let clock = configuration_clock(
        ConfigurationLoadScenario::JsonResolutionFailure,
        Duration::from_millis(1),
    )?;
    let capture = DiscoverySpanCapture::default();
    let subscriber =
        tracing_subscriber::registry().with(capture.clone().with_filter(LevelFilter::TRACE));

    let result =
        tracing::subscriber::with_default(subscriber, || resolve_configuration(&context, &clock));
    ensure!(
        result.is_err(),
        "the missing explicit configuration must fail"
    );
    let fields = capture.fields();
    ensure!(
        fields.contains(&"outcome=\"error\"".to_owned()),
        "startup must record an error discovery outcome: {fields:?}"
    );
    ensure!(
        fields.contains(&"error_category=\"file\"".to_owned()),
        "startup must record the bounded file error category: {fields:?}"
    );
    Ok(())
}
