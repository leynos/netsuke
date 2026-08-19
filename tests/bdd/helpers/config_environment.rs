//! Injected configuration environment for in-process BDD CLI merges.

use std::ffi::OsString;

use clap::ArgMatches;
use netsuke::cli::{Cli, ConfigEnvProvider};
use ortho_config::OrthoResult;

use crate::bdd::fixtures::TestWorld;

struct ScenarioEnvironment {
    entries: Vec<(OsString, OsString)>,
}

impl ScenarioEnvironment {
    fn from_world(world: &TestWorld) -> Self {
        let entries = world
            .env_vars_forward
            .borrow()
            .iter()
            .map(|(key, value)| (OsString::from(key), value.clone()))
            .collect();
        Self { entries }
    }
}

impl ConfigEnvProvider for ScenarioEnvironment {
    fn get(&self, key: &str) -> Option<OsString> {
        self.entries
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(key))
            .map(|(_, value)| value.clone())
    }

    fn entries(&self) -> Vec<(OsString, OsString)> {
        self.entries.clone()
    }
}

/// Merge CLI configuration using only the environment recorded in `world`.
pub fn merge_with_world_env(
    world: &TestWorld,
    cli: &Cli,
    matches: &ArgMatches,
) -> OrthoResult<Cli> {
    netsuke::cli::merge_with_config_and_env(cli, matches, &ScenarioEnvironment::from_world(world))
}
