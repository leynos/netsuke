//! Bind the early JSON resolver to the merge's environment-key handling.
//!
//! The merge normalizes every `NETSUKE_`-prefixed key, so on a case-sensitive
//! system `netsuke_json=1` reaches it as `json = true`. The early resolver runs
//! before that merge, to choose the output mode for its diagnostics. If it read
//! only the canonical spelling, a merge failure would print human diagnostics
//! although the merged configuration selects JSON. These tests compare the two
//! decisions for the same injected environment, including the failing-merge
//! case that makes the disagreement observable.

use super::{JSON_ENV_VAR, resolve_json_and_layers_outcome_with_env};
use crate::cli::{Cli, merge_with_cached_file_layers, test_support::TestEnv};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::{ArgMatches, CommandFactory};
use rstest::rstest;
use tempfile::tempdir;

/// Build a CLI and matches that select a temporary config file.
///
/// The returned directory owns the file, so callers must keep it alive for as
/// long as they use the CLI.
fn config_candidate(contents: &str) -> Result<(tempfile::TempDir, Cli, ArgMatches)> {
    let dir = tempdir()?;
    let config_path = dir.path().join("netsuke.toml");
    let config_dir = Dir::open_ambient_dir(dir.path(), ambient_authority())?;
    config_dir.write("netsuke.toml", contents)?;
    let cli = Cli {
        config: Some(config_path),
        ..Cli::default()
    };
    let matches = Cli::command().get_matches_from(["netsuke"]);
    Ok((dir, cli, matches))
}

/// Each spelling the merge accepts must select the same mode in both places.
#[rstest]
#[case::canonical(JSON_ENV_VAR)]
#[case::lower_case("netsuke_json")]
#[case::mixed_case("NetSuke_Json")]
#[case::surrounding_whitespace(" NETSUKE_JSON ")]
fn an_environment_alias_selects_the_same_json_mode_as_the_merge(
    #[case] key: &'static str,
) -> Result<()> {
    let (_dir, cli, matches) = config_candidate("json = false\n")?;
    let env = TestEnv::default().with_var(key, "1");

    let (early, outcome) = resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    let is_json = early.with_context(|| format!("the early resolver must accept {key:?}"))?;
    ensure!(is_json, "the early resolver must select JSON for {key:?}");

    let merged = merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers())
        .with_context(|| format!("the merge must accept {key:?}"))?;
    ensure!(
        merged.json,
        "the merge must select JSON for {key:?}, matching the early resolver"
    );
    Ok(())
}

/// A rejected merge must still be reported in the mode the alias selects.
#[rstest]
#[case::canonical(JSON_ENV_VAR)]
#[case::lower_case("netsuke_json")]
fn a_normalized_alias_selects_json_when_the_merge_fails(#[case] key: &'static str) -> Result<()> {
    // `jobs` accepts only an integer, so the quoted string is a rejection the
    // merge cannot ignore; the alias supplies a valid `json` value, so the
    // rejection is not about the diagnostic mode. The rejection's exact shape
    // belongs to the merge's own tests; what this case pins is that the mode
    // was already decided in favour of JSON when that rejection is reported.
    let (_dir, cli, matches) = config_candidate("json = false\njobs = \"many\"\n")?;
    let env = TestEnv::default().with_var(key, "1");

    let (early, outcome) = resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    ensure!(
        early.with_context(|| format!("the early resolver must accept {key:?}"))?,
        "the early resolver must select JSON for {key:?} before the merge fails"
    );

    merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers())
        .expect_err("the quoted jobs value must fail the merge");
    Ok(())
}

/// Keys naming other paths must not be read as the JSON preference.
///
/// Only the exact `json` path counts. A merge may reject one of these keys,
/// but that rejection concerns the key itself; it cannot be reported as a JSON
/// selection, so the early resolver must keep the file's preference.
#[rstest]
#[case::suffix("NETSUKE_JSONX")]
#[case::nested("NETSUKE_JSON__NESTED")]
#[case::unrelated("NETSUKE_VERBOSE")]
fn other_environment_keys_leave_the_file_preference_in_place(
    #[case] key: &'static str,
) -> Result<()> {
    let (_dir, cli, matches) = config_candidate("json = false\n")?;
    let env = TestEnv::default().with_var(key, "1");

    let (early, outcome) = resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    ensure!(
        !early?,
        "{key:?} must not select the JSON mode the merged file leaves disabled"
    );

    if let Ok(merged) = merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers()) {
        ensure!(!merged.json, "a merged {key:?} must not enable JSON either");
    }
    Ok(())
}
