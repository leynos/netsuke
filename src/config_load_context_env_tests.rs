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
use std::time::Duration;

/// In-memory configuration environment for startup-metrics test scenarios.
pub(super) struct EmptyConfigEnv;

impl cli::ConfigEnvProvider for EmptyConfigEnv {
    fn get(&self, _key: &str) -> Option<OsString> {
        None
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

    let merged = resolve_configuration(&context, &clock)
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
    Ok(())
}
