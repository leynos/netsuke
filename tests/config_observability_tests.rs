//! Binary-level configuration observability integration tests.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use rstest::rstest;
use test_support::check_ninja::fake_ninja_check_build_file;
use test_support::config_metrics::{MetricSnapshotRecord, assert_config_metrics_snapshot};
use test_support::fixture::setup_minimal_workspace;
use test_support::fs as test_fs;
use test_support::netsuke::run_netsuke_in_with_env;

/// Captured output needed by configuration-observability assertions.
struct CommandOutput {
    stderr: String,
    success: bool,
}

fn run_config_layer_build(
    context: &str,
    config_content: &str,
    args: &[&str],
    extra_env: &[(&str, &str)],
) -> Result<CommandOutput> {
    let workspace = setup_minimal_workspace(Utf8Path::new(env!("CARGO_MANIFEST_DIR")), context)?;
    test_fs::write(workspace.path().join(".netsuke.toml"), config_content)
        .context("write config file")?;
    let (_ninja_dir, ninja_path) = fake_ninja_check_build_file()?;
    let ninja = ninja_path.to_string_lossy().into_owned();
    let mut environment = extra_env.to_vec();
    environment.push(("NETSUKE_NINJA", ninja.as_str()));
    let run = run_netsuke_in_with_env(workspace.path(), args, &environment)?;
    Ok(CommandOutput {
        stderr: run.stderr,
        success: run.success,
    })
}

const SUCCESSFUL_CONFIG_METRICS: &[MetricSnapshotRecord] = &[
    MetricSnapshotRecord {
        name: "config_load_total",
        labels: &[
            "Label(\"phase\", \"diag_mode\")",
            "Label(\"outcome\", \"success\")",
        ],
        value: Some("Counter(1)"),
    },
    MetricSnapshotRecord {
        name: "config_load_total",
        labels: &[
            "Label(\"phase\", \"merge\")",
            "Label(\"outcome\", \"success\")",
        ],
        value: Some("Counter(1)"),
    },
    MetricSnapshotRecord {
        name: "config_load_duration_seconds",
        labels: &["Label(\"phase\", \"diag_mode\")"],
        value: None,
    },
    MetricSnapshotRecord {
        name: "config_load_duration_seconds",
        labels: &["Label(\"phase\", \"merge\")"],
        value: None,
    },
];

#[rstest]
#[case::config_file_enables("verbose = true\n", &["build"], &[], true)]
#[case::env_var_enables(
    "verbose = false\n",
    &["build"],
    &[("NETSUKE_VERBOSE", "true")],
    true
)]
#[case::env_var_overrides_config(
    "verbose = true\n",
    &["build"],
    &[("NETSUKE_VERBOSE", "false")],
    false
)]
#[case::cli_flag_overrides_env(
    "verbose = true\n",
    &["--verbose", "build"],
    &[("NETSUKE_VERBOSE", "false")],
    true
)]
fn verbose_config_precedence(
    #[case] config_content: &str,
    #[case] args: &[&str],
    #[case] extra_env: &[(&str, &str)],
    #[case] expect_verbose_diagnostics: bool,
) -> Result<()> {
    let output =
        run_config_layer_build("verbose config precedence", config_content, args, extra_env)?;
    ensure!(output.success, "expected build to succeed");
    if expect_verbose_diagnostics {
        ensure!(
            output.stderr.contains("Timing"),
            "expected verbose timing summary in stderr, got:\n{}",
            output.stderr
        );
        assert_config_metrics_snapshot(&output.stderr, SUCCESSFUL_CONFIG_METRICS)?;
    } else {
        ensure!(
            !output.stderr.contains("Timing"),
            "expected no timing summary, got:\n{}",
            output.stderr
        );
        ensure!(
            !output.stderr.contains("metrics snapshot"),
            "expected no metrics snapshot, got:\n{}",
            output.stderr
        );
    }
    Ok(())
}

/// An invalid enum value in a config file produces a bounded merge failure.
#[test]
fn invalid_config_value_reports_bounded_merge_failure() -> Result<()> {
    let workspace = setup_minimal_workspace(
        Utf8Path::new(env!("CARGO_MANIFEST_DIR")),
        "invalid config value",
    )?;
    test_fs::write(workspace.path().join(".netsuke.toml"), "color = \"loud\"\n")
        .context("write invalid config file")?;
    let run = run_netsuke_in_with_env(workspace.path(), &["generate"], &[])?;

    ensure!(
        !run.success,
        "expected generate with invalid config to fail"
    );
    ensure!(
        run.stderr.contains("configuration load failed")
            && run.stderr.contains("operation=\"config_merge\"")
            && run.stderr.contains("error_category=\"parse\""),
        "expected a human-mode config merge failure record, got:\n{}",
        run.stderr
    );
    Ok(())
}
