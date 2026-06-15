//! Tests for configuration discovery tracing.

use super::*;
use anyhow::{Context, Result, ensure};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;
use test_support::{EnvVarGuard, env_lock::EnvLock};
use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{
    Layer, filter::LevelFilter, layer::Context as LayerContext, prelude::*, registry::LookupSpan,
};

#[derive(Debug, Clone, Default)]
struct CapturedEvents {
    fields: Arc<Mutex<Vec<String>>>,
}

impl CapturedEvents {
    fn snapshot(&self) -> Vec<String> {
        self.fields.lock().expect("captured events lock").clone()
    }
}

#[derive(Debug, Clone, Default)]
struct CapturedEventsLayer {
    events: Arc<Mutex<Vec<String>>>,
}

impl<S> Layer<S> for CapturedEventsLayer
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: LayerContext<'_, S>) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        self.events
            .lock()
            .expect("captured events lock")
            .push(visitor.fields.join(" "));
    }
}

#[derive(Debug, Default)]
struct FieldVisitor {
    fields: Vec<String>,
}

impl Visit for FieldVisitor {
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields.push(format!("{}={value:?}", field.name()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields.push(format!("{}={value:?}", field.name()));
    }
}

fn with_test_subscriber<T>(test: impl FnOnce(CapturedEvents) -> T) -> T {
    let layer = CapturedEventsLayer::default();
    let captured = CapturedEvents {
        fields: Arc::clone(&layer.events),
    };
    let subscriber = tracing_subscriber::registry().with(layer.with_filter(LevelFilter::TRACE));
    tracing::subscriber::with_default(subscriber, || test(captured))
}

fn find_event<'a>(events: &'a [String], message: &str) -> Result<&'a String> {
    events
        .iter()
        .find(|event| event.contains(message))
        .with_context(|| format!("expected event containing {message:?} in {events:?}"))
}

#[test]
fn explicit_config_path_logs_selected_cli_path() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let _config_guard = EnvVarGuard::remove(CONFIG_ENV_VAR);
    let _legacy_guard = EnvVarGuard::remove(CONFIG_ENV_VAR_LEGACY);
    let selected_path = PathBuf::from("selected.toml");
    let cli = Cli {
        config: Some(selected_path.clone()),
        ..Cli::default()
    };

    with_test_subscriber(|captured| {
        let resolved = explicit_config_path(&cli);
        let events = captured.snapshot();
        let event = find_event(&events, "resolved config path")?;

        ensure!(
            resolved == Some(selected_path),
            "expected CLI config path to resolve"
        );
        ensure!(
            event.contains("selector=\"cli_flag\""),
            "selector field should identify CLI flag: {event}"
        );
        ensure!(
            event.contains("path=Some(\"selected.toml\")"),
            "path field should contain selected path: {event}"
        );
        Ok(())
    })
}

#[test]
fn explicit_config_path_logs_env_lookup_and_selector() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let _config_guard = EnvVarGuard::set(CONFIG_ENV_VAR, "env.toml");
    let _legacy_guard = EnvVarGuard::set(CONFIG_ENV_VAR_LEGACY, "legacy.toml");

    with_test_subscriber(|captured| {
        let resolved = explicit_config_path(&Cli::default());
        let events = captured.snapshot();
        let env_event = events
            .iter()
            .find(|event| {
                event.contains("read config path variable")
                    && event.contains("var_name=\"NETSUKE_CONFIG\"")
            })
            .with_context(|| format!("expected NETSUKE_CONFIG trace event in {events:?}"))?;
        let selector_event = find_event(&events, "resolved config path")?;

        ensure!(
            resolved == Some(PathBuf::from("env.toml")),
            "NETSUKE_CONFIG should win over legacy selector"
        );
        ensure!(
            env_event.contains("found=true"),
            "env trace should record that a path was found: {env_event}"
        );
        ensure!(
            selector_event.contains("selector=\"NETSUKE_CONFIG\""),
            "selector should identify NETSUKE_CONFIG: {selector_event}"
        );
        Ok(())
    })
}

#[test]
fn collect_diag_file_layers_logs_explicit_path_branch() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let config_path = temp.path().join("config.toml");
    std::fs::write(&config_path, "theme = \"ascii\"\n")
        .with_context(|| format!("write {}", config_path.display()))?;
    let cli = Cli {
        config: Some(config_path),
        ..Cli::default()
    };

    with_test_subscriber(|captured| {
        let layers = collect_diag_file_layers(&cli)?;
        let events = captured.snapshot();
        let branch_event = find_event(&events, "using explicit config path")?;

        ensure!(!layers.is_empty(), "explicit config should load layers");
        ensure!(
            branch_event.contains("path="),
            "explicit path branch should include a path field: {branch_event}"
        );
        Ok(())
    })
}

#[test]
fn collect_diag_file_layers_logs_discovery_branch() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let _config_guard = EnvVarGuard::remove(CONFIG_ENV_VAR);
    let _legacy_guard = EnvVarGuard::remove(CONFIG_ENV_VAR_LEGACY);

    with_test_subscriber(|captured| {
        let layers = collect_diag_file_layers(&Cli::default())?;
        let events = captured.snapshot();
        let branch_event = find_event(&events, "using config discovery")?;

        ensure!(layers.is_empty(), "default workspace should not add layers");
        ensure!(
            branch_event.contains("message=using config discovery"),
            "discovery branch should emit the expected event: {branch_event}"
        );
        Ok(())
    })
}

#[test]
fn load_layers_from_path_logs_bounded_failure_fields() -> Result<()> {
    let missing_path = PathBuf::from("missing-secret-name.toml");

    with_test_subscriber(|captured| {
        let error = load_layers_from_path(&missing_path)
            .expect_err("missing explicit config file should fail");
        let events = captured.snapshot();
        let warn_event = find_event(&events, "explicit config load failed")?;
        let path_hash = short_hash(missing_path.to_string_lossy().as_bytes());

        ensure!(
            error.to_string().contains("missing-secret-name.toml"),
            "returned error should retain the diagnostic path"
        );
        ensure!(
            warn_event.contains("failure_kind=Missing"),
            "warn event should include bounded failure kind: {warn_event}"
        );
        ensure!(
            warn_event.contains(&format!("path_hash={path_hash}")),
            "warn event should include path hash: {warn_event}"
        );
        ensure!(
            warn_event.contains("path_file_name=Some(\"missing-secret-name.toml\")"),
            "warn event should include only the file name for path correlation: {warn_event}"
        );
        ensure!(
            !warn_event.contains("error="),
            "warn event should not include full formatted error text: {warn_event}"
        );
        Ok(())
    })
}
