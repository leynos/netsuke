//! Direct observer tests for bounded configuration-merge events.
//!
//! These tests preserve the event API's privacy boundary without relying on a
//! tracing subscriber or rendered diagnostics.

use anyhow::{Context, Result, ensure};
use netsuke::cli::MergeEvent;
use rstest::rstest;
use std::ffi::OsString;

use super::merge_logging::{TestEnv, merge_and_observe, merge_and_observe_after_json_resolution};

const JOBS_REASON: &str = "job count is outside the supported range";
const NO_INPUT_REASON: &str =
    "no_input = false is unsupported because Netsuke has no interactive mode";

#[rstest]
fn observer_reports_non_empty_file_layer_with_bounded_path() -> Result<()> {
    let temp_dir = tempfile::tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    test_support::fs::write(&config_path, "jobs = 2\n").context("write valid config file")?;
    let config_arg = config_path.to_string_lossy().into_owned();
    let (events, merge_ok) =
        merge_and_observe(&["netsuke", "--config", &config_arg], &TestEnv::default())?;

    ensure!(merge_ok, "a valid file layer should merge successfully");
    let [
        MergeEvent::DefaultsApplied,
        MergeEvent::FileLayersCollected { layer_count: 1 },
        MergeEvent::FileLayerApplied {
            path_hash: Some(path_hash),
        },
        MergeEvent::EnvironmentApplied { is_empty: true },
        MergeEvent::CliOverridesAbsent,
        MergeEvent::FetchPolicyReconciled { .. },
    ] = events.as_slice()
    else {
        anyhow::bail!("expected one bounded file-layer event: {events:#?}");
    };
    ensure!(
        path_hash.len() == 16 && path_hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "file-layer event must expose a bounded path hash: {path_hash:?}"
    );
    Ok(())
}

#[rstest]
fn observer_reports_non_empty_environment_without_value() -> Result<()> {
    let env = TestEnv {
        entries: vec![(
            OsString::from("NETSUKE_LOCALE"),
            OsString::from("private-locale-value"),
        )],
    };
    let (events, merge_ok) = merge_and_observe(&["netsuke"], &env)?;

    ensure!(
        merge_ok,
        "a valid environment layer should merge successfully"
    );
    ensure!(
        matches!(
            events.as_slice(),
            [
                MergeEvent::DefaultsApplied,
                MergeEvent::FileLayersCollected { layer_count: 0 },
                MergeEvent::EnvironmentApplied { is_empty: false },
                MergeEvent::CliOverridesAbsent,
                MergeEvent::FetchPolicyReconciled { .. },
            ]
        ),
        "environment events must retain only their empty-state flag: {events:#?}"
    );
    Ok(())
}

#[rstest]
fn observer_reports_precise_cli_override_leaf_keys() -> Result<()> {
    let target = "private-build-target";
    let (events, merge_ok) = merge_and_observe(
        &["netsuke", "--jobs", "2", "build", target],
        &TestEnv::default(),
    )?;

    ensure!(merge_ok, "CLI overrides should merge successfully");
    let [
        MergeEvent::DefaultsApplied,
        MergeEvent::FileLayersCollected { layer_count: 0 },
        MergeEvent::EnvironmentApplied { is_empty: true },
        MergeEvent::CliOverridesApplied { override_keys },
        MergeEvent::FetchPolicyReconciled { .. },
    ] = events.as_slice()
    else {
        anyhow::bail!("expected bounded CLI override event: {events:#?}");
    };
    ensure!(
        override_keys
            .iter()
            .map(String::as_str)
            .eq(["jobs", "cmds.build.targets"]),
        "CLI event must retain exactly its leaf keys: {override_keys:#?}"
    );
    Ok(())
}

#[rstest]
#[case::jobs("jobs = 0\n", "jobs", JOBS_REASON)]
#[case::no_input("no_input = false\n", "no_input", NO_INPUT_REASON)]
fn observer_reports_validation_rejection_fields(
    #[case] config: &str,
    #[case] expected_key: &str,
    #[case] expected_reason: &str,
) -> Result<()> {
    let temp_dir = tempfile::tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    test_support::fs::write(&config_path, config).context("write invalid config file")?;
    let config_arg = config_path.to_string_lossy().into_owned();
    let (events, merge_ok) =
        merge_and_observe(&["netsuke", "--config", &config_arg], &TestEnv::default())?;

    ensure!(!merge_ok, "invalid configuration must fail the merge");
    let validation_events = events.iter().filter_map(|event| match event {
        MergeEvent::ValidationRejected { key, reason } => Some((key.as_str(), *reason)),
        _ => None,
    });
    ensure!(
        validation_events.eq([(expected_key, expected_reason)].iter().copied()),
        "validation event must retain its fixed key and reason: {events:#?}"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn observer_reports_malformed_environment_failure() -> Result<()> {
    use std::os::unix::ffi::OsStringExt;

    let env = TestEnv {
        entries: vec![(
            OsString::from_vec(vec![b'N', b'E', b'T', b'S', b'U', b'K', b'E', b'_', 0xff]),
            OsString::from("value"),
        )],
    };
    let (events, merge_ok) = merge_and_observe(&["netsuke"], &env)?;

    ensure!(!merge_ok, "malformed environment input must fail the merge");
    ensure!(
        matches!(
            events.as_slice(),
            [
                MergeEvent::DefaultsApplied,
                MergeEvent::FileLayersCollected { layer_count: 0 },
                MergeEvent::EnvironmentFailed,
                MergeEvent::CliOverridesAbsent,
            ]
        ),
        "environment failure must not retain the malformed key or value: {events:#?}"
    );
    Ok(())
}

#[rstest]
fn observer_reports_retained_file_collection_failure() -> Result<()> {
    let temp_dir = tempfile::tempdir().context("create temporary config directory")?;
    let missing_config = temp_dir.path().join("missing-netsuke.toml");
    let config_arg = missing_config.to_string_lossy().into_owned();
    let (json_mode_resolved, events, merge_ok) = merge_and_observe_after_json_resolution(
        &["netsuke", "--config", &config_arg],
        &TestEnv::default(),
    )?;

    ensure!(
        !json_mode_resolved,
        "missing explicit config should fail early JSON resolution"
    );
    ensure!(
        !merge_ok,
        "retained discovery errors should fail the cached merge"
    );
    ensure!(
        matches!(
            events.as_slice(),
            [
                MergeEvent::DefaultsApplied,
                MergeEvent::FileLayerCollectionFailed { error_count: 1 },
                MergeEvent::EnvironmentApplied { is_empty: true },
                MergeEvent::CliOverridesAbsent,
            ]
        ),
        "the cached error hand-off must retain only the collection failure: {events:#?}"
    );
    Ok(())
}
