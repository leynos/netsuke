//! Property-based tests for CLI configuration merge invariants.
//!
//! Generative coverage for the invariants established by the layered merge
//! pipeline: layer precedence ordering (defaults → file → environment →
//! CLI), list-appending semantics, and cross-field validation rules.

use netsuke::cli::CliConfig;
use ortho_config::{MergeComposer, sanitize_value};
use proptest::prelude::*;
use serde_json::json;

const MAX_JOBS: usize = 64;

/// Merge an optional jobs value per layer through the real pipeline.
fn merge_jobs_layers(
    file: Option<usize>,
    env: Option<usize>,
    cli: Option<usize>,
) -> Result<CliConfig, TestCaseError> {
    let defaults = sanitize_value(&CliConfig::default())
        .map_err(|e| TestCaseError::fail(format!("sanitize defaults: {e}")))?;
    let mut composer = MergeComposer::new();
    composer.push_defaults(defaults);
    if let Some(jobs) = file {
        composer.push_file(json!({ "jobs": jobs }), None);
    }
    if let Some(jobs) = env {
        composer.push_environment(json!({ "jobs": jobs }));
    }
    if let Some(jobs) = cli {
        composer.push_cli(json!({ "jobs": jobs }));
    }
    CliConfig::merge_from_layers(composer.layers())
        .map_err(|e| TestCaseError::fail(format!("merge failed: {e}")))
}

fn merge_lists(
    file: &[String],
    env: &[String],
    cli: &[String],
) -> Result<CliConfig, TestCaseError> {
    let defaults = sanitize_value(&CliConfig::default())
        .map_err(|e| TestCaseError::fail(format!("sanitize defaults: {e}")))?;
    let mut composer = MergeComposer::new();
    composer.push_defaults(defaults);
    composer.push_file(
        json!({ "fetch_allow_scheme": file, "cmds": { "build": { "targets": file } } }),
        None,
    );
    composer.push_environment(json!({ "fetch_allow_scheme": env }));
    composer.push_cli(json!({ "fetch_allow_scheme": cli }));
    CliConfig::merge_from_layers(composer.layers())
        .map_err(|e| TestCaseError::fail(format!("merge failed: {e}")))
}

fn merge_file_layer(file_layer: serde_json::Value) -> ortho_config::OrthoResult<CliConfig> {
    let mut composer = MergeComposer::new();
    if let Ok(defaults) = sanitize_value(&CliConfig::default()) {
        composer.push_defaults(defaults);
    }
    composer.push_file(file_layer, None);
    CliConfig::merge_from_layers(composer.layers())
}

fn jobs_value() -> impl Strategy<Value = usize> {
    1..=MAX_JOBS
}

fn scheme_list(max: usize) -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec("[a-z]{2,6}", 0..=max)
}

fn theme_option() -> impl Strategy<Value = Option<&'static str>> {
    prop_oneof![
        Just(None),
        Just(Some("auto")),
        Just(Some("unicode")),
        Just(Some("ascii")),
    ]
}

fn spinner_option() -> impl Strategy<Value = Option<&'static str>> {
    prop_oneof![
        Just(None),
        Just(Some("auto")),
        Just(Some("enabled")),
        Just(Some("disabled")),
    ]
}

fn tristate() -> impl Strategy<Value = Option<bool>> {
    prop_oneof![Just(None), Just(Some(true)), Just(Some(false))]
}

proptest! {
    /// The highest-precedence layer that supplies `jobs` always wins:
    /// CLI over environment over file.
    #[test]
    fn jobs_follow_layer_precedence(
        file in proptest::option::of(jobs_value()),
        env in proptest::option::of(jobs_value()),
        cli in proptest::option::of(jobs_value()),
    ) {
        let merged = merge_jobs_layers(file, env, cli)?;
        let expected = cli.or(env).or(file);
        prop_assert_eq!(merged.jobs, expected);
    }

    /// Append-merged lists concatenate in layer order: file, then
    /// environment, then CLI.
    #[test]
    fn append_lists_concatenate_in_layer_order(
        file in scheme_list(4),
        env in scheme_list(4),
        cli in scheme_list(4),
    ) {
        let merged = merge_lists(&file, &env, &cli)?;
        let expected: Vec<String> = file
            .iter()
            .chain(env.iter())
            .chain(cli.iter())
            .cloned()
            .collect();
        prop_assert_eq!(merged.fetch_allow_scheme, expected);
        // Build targets supplied by the file layer survive the merge intact.
        prop_assert_eq!(merged.cmds.build.targets, file);
    }

    /// The theme/no_emoji validation fires for exactly the two documented
    /// conflicting combinations across the full domain.
    #[test]
    fn theme_no_emoji_validation_matches_documented_rules(
        theme in theme_option(),
        no_emoji in tristate(),
    ) {
        let mut layer = serde_json::Map::new();
        if let Some(theme_name) = theme {
            layer.insert("theme".into(), json!(theme_name));
        }
        if let Some(no_emoji_flag) = no_emoji {
            layer.insert("no_emoji".into(), json!(no_emoji_flag));
        }
        let outcome = merge_file_layer(serde_json::Value::Object(layer));
        let conflict = matches!(
            (theme, no_emoji),
            (Some("unicode"), Some(true)) | (Some("ascii"), Some(false))
        );
        prop_assert_eq!(
            outcome.is_err(),
            conflict,
            "theme={:?} no_emoji={:?} outcome={:?}",
            theme,
            no_emoji,
            outcome.err().map(|e| e.to_string())
        );
    }

    /// The spinner_mode/progress validation fires for exactly the two
    /// documented conflicting combinations across the full domain.
    #[test]
    fn spinner_progress_validation_matches_documented_rules(
        spinner in spinner_option(),
        progress in tristate(),
    ) {
        let mut layer = serde_json::Map::new();
        if let Some(spinner_name) = spinner {
            layer.insert("spinner_mode".into(), json!(spinner_name));
        }
        if let Some(progress_flag) = progress {
            layer.insert("progress".into(), json!(progress_flag));
        }
        let outcome = merge_file_layer(serde_json::Value::Object(layer));
        let conflict = matches!(
            (spinner, progress),
            (Some("disabled"), Some(true)) | (Some("enabled"), Some(false))
        );
        prop_assert_eq!(
            outcome.is_err(),
            conflict,
            "spinner={:?} progress={:?} outcome={:?}",
            spinner,
            progress,
            outcome.err().map(|e| e.to_string())
        );
    }

    /// `jobs` is accepted exactly within `1..=MAX_JOBS`.
    #[test]
    fn jobs_bounds_validation_matches_documented_rules(jobs in 0_usize..=(MAX_JOBS * 2)) {
        let outcome = merge_file_layer(json!({ "jobs": jobs }));
        let out_of_bounds = jobs == 0 || jobs > MAX_JOBS;
        prop_assert_eq!(
            outcome.is_err(),
            out_of_bounds,
            "jobs={} outcome={:?}",
            jobs,
            outcome.err().map(|e| e.to_string())
        );
    }
}
