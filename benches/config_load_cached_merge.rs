//! Benchmark cached configuration loading without complete layer copies.
//!
//! The benchmark resolves early JSON mode and then merges the same cached
//! layers from a large nested configuration payload. It guards the ownership
//! transfer that avoids cloning complete `MergeLayer` values before merging.

#![feature(test)]

extern crate test;

use cap_std::{ambient_authority, fs::Dir};
use clap::CommandFactory;
use netsuke::cli::{
    Cli, ConfigEnvProvider, merge_with_cached_file_layers, resolve_json_and_layers_outcome_with_env,
};
use std::ffi::OsString;
use tempfile::TempDir;
use test::{Bencher, black_box};

/// Empty environment provider for a deterministic file-layer benchmark.
struct BenchmarkEnv;

impl ConfigEnvProvider for BenchmarkEnv {
    fn get(&self, _key: &str) -> Option<OsString> {
        None
    }
}

/// Create a large nested configuration payload with valid build targets.
fn large_configuration() -> String {
    let targets = (0..4_096)
        .map(|index| format!("\"target-{index:04}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("json = true\n\n[cmds.build]\ntargets = [{targets}]\n")
}

/// Create an explicit configuration file retained for the entire benchmark.
fn benchmark_configuration() -> anyhow::Result<(TempDir, Cli, clap::ArgMatches)> {
    let temp = tempfile::tempdir()?;
    let config_path = temp.path().join("netsuke.toml");
    let config_dir = Dir::open_ambient_dir(temp.path(), ambient_authority())?;
    config_dir.write("netsuke.toml", large_configuration())?;
    let cli = Cli {
        config: Some(config_path),
        ..Cli::default()
    };
    let matches = Cli::command().get_matches_from(["netsuke"]);
    Ok((temp, cli, matches))
}

/// Benchmark one discovery-plus-cached-merge configuration load.
#[bench]
fn resolves_json_then_merges_cached_large_config(bencher: &mut Bencher) {
    let (_temp, cli, matches) = benchmark_configuration()
        .unwrap_or_else(|error| panic!("create benchmark configuration: {error}"));
    let env = BenchmarkEnv;

    bencher.iter(|| {
        let (json_result, outcome) = resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
        let json =
            json_result.unwrap_or_else(|error| panic!("resolve benchmark JSON mode: {error}"));
        let merged = merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers())
            .unwrap_or_else(|error| panic!("merge benchmark configuration: {error}"));
        black_box((json, merged));
    });
}
