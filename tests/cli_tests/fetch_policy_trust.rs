//! End-to-end trust-boundary tests for layered fetch-policy configuration.

use super::merge_probe::{environment_with_system_scope, merge_in_child};
use anyhow::{Context, Result, ensure};
use netsuke::cli::Cli;
use netsuke::stdlib::NetworkPolicyViolation;
use rstest::rstest;
use std::ffi::OsString;
use std::path::Path;
use tempfile::tempdir;
use test_support::fs as test_fs;
use url::Url;

/// Describe the configuration-file contents used in one merge scenario.
#[derive(Default)]
struct FetchPolicyFileLayers<'a> {
    /// Hold system-scope configuration contents.
    system: Option<&'a str>,
    /// Hold user-scope configuration contents.
    user: Option<&'a str>,
    /// Hold primary project-scope configuration contents.
    project: Option<&'a str>,
}

/// Write a configuration file while creating its parent directory.
fn write_config(config_path: impl AsRef<Path>, contents: &str) -> Result<()> {
    let target = config_path.as_ref();
    let parent = target
        .parent()
        .context("configuration file has no parent")?;
    test_fs::create_dir_all(parent).context("create configuration directory")?;
    test_fs::write(target, contents).with_context(|| format!("write {}", target.display()))
}

/// Merge one isolated system, user, project, environment, and CLI scenario.
fn merge_fetch_policy(
    file_layers: &FetchPolicyFileLayers<'_>,
    environment_overrides: &[(OsString, OsString)],
    args: &[&str],
) -> Result<Cli> {
    let project = tempdir().context("create project directory")?;
    let home = tempdir().context("create home directory")?;
    let system = tempdir().context("create system directory")?;
    if let Some(config) = file_layers.system {
        write_config(system.path().join("netsuke/config.toml"), config)?;
    }
    if let Some(config) = file_layers.user {
        write_config(home.path().join(".config/netsuke/config.toml"), config)?;
    }
    if let Some(config) = file_layers.project {
        write_config(project.path().join(".netsuke.toml"), config)?;
    }
    let mut environment = environment_with_system_scope(home.path(), system.path(), &[]);
    environment.extend_from_slice(environment_overrides);
    merge_in_child(args, project.path(), &environment)
}

/// Build a URL for a policy assertion.
fn url(value: &str) -> Result<Url> {
    Url::parse(value).with_context(|| format!("parse test URL {value}"))
}

/// Evaluate a URL and return the policy rejection it produces.
fn rejection(cli: &Cli, value: &str) -> Result<NetworkPolicyViolation> {
    cli.network_policy()?
        .evaluate(&url(value)?)
        .err()
        .with_context(|| format!("URL should be rejected by the policy: {value}"))
}

/// Assert that a URL is rejected because its host was not allowlisted.
fn assert_host_not_allowlisted(cli: &Cli, value: &str) -> Result<()> {
    let error = rejection(cli, value)?;
    ensure!(
        matches!(error, NetworkPolicyViolation::HostNotAllowlisted { .. }),
        "expected host allowlist violation, got {error:?}"
    );
    Ok(())
}

/// Assert that a URL is rejected because its scheme was not allowlisted.
fn assert_scheme_not_allowed(cli: &Cli, value: &str) -> Result<()> {
    let error = rejection(cli, value)?;
    ensure!(
        matches!(error, NetworkPolicyViolation::SchemeNotAllowed { .. }),
        "expected scheme violation, got {error:?}"
    );
    Ok(())
}

/// Assert that a URL is rejected because a block rule takes priority.
fn assert_host_blocked(cli: &Cli, value: &str) -> Result<()> {
    let error = rejection(cli, value)?;
    ensure!(
        matches!(error, NetworkPolicyViolation::HostBlocked { .. }),
        "expected host block violation, got {error:?}"
    );
    Ok(())
}

const PROJECT_GRANTS: &str = r#"
fetch_default_deny = false
fetch_allow_scheme = ["http"]
fetch_allow_host = ["169.254.169.254"]
"#;

const PROJECT_TIGHTENING: &str = "fetch_default_deny = true";

#[rstest]
fn defaults_keep_project_fetch_grants_outside_operator_policy() -> Result<()> {
    let merged = merge_fetch_policy(
        &FetchPolicyFileLayers {
            project: Some(PROJECT_GRANTS),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;

    ensure!(
        !merged.fetch_default_deny,
        "a project false request should not alter the default operator policy"
    );
    assert_scheme_not_allowed(&merged, "http://169.254.169.254")
}

#[rstest]
fn project_can_tighten_default_deny_without_operator_policy() -> Result<()> {
    let merged = merge_fetch_policy(
        &FetchPolicyFileLayers {
            project: Some(PROJECT_TIGHTENING),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;

    ensure!(
        merged.fetch_default_deny,
        "a project true request should tighten the default operator policy"
    );
    assert_host_not_allowlisted(&merged, "https://downloads.example.org")
}

#[rstest]
fn project_cannot_downgrade_user_default_deny_or_add_schemes() -> Result<()> {
    let user_config = r#"
fetch_default_deny = true
fetch_allow_host = ["downloads.example.org"]
"#;
    let merged = merge_fetch_policy(
        &FetchPolicyFileLayers {
            user: Some(user_config),
            project: Some(PROJECT_GRANTS),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;

    ensure!(
        merged.fetch_default_deny,
        "project false request must not clear user deny"
    );
    ensure!(
        merged
            .network_policy()?
            .evaluate(&url("https://downloads.example.org")?)
            .is_ok(),
        "user allowlist entry should remain effective"
    );
    assert_host_not_allowlisted(&merged, "https://169.254.169.254")?;
    assert_scheme_not_allowed(&merged, "http://downloads.example.org")
}

#[rstest]
fn project_cannot_widen_system_environment_or_cli_operator_policy() -> Result<()> {
    let system_config = r#"
fetch_default_deny = true
fetch_allow_host = ["system.example.org"]
fetch_block_host = ["blocked.example.org"]
"#;
    let environment = [
        (
            OsString::from("NETSUKE_FETCH_ALLOW_HOST"),
            OsString::from("environment.example.org"),
        ),
        (
            OsString::from("NETSUKE_FETCH_BLOCK_HOST"),
            OsString::from("environment-blocked.example.org"),
        ),
    ];
    let merged = merge_fetch_policy(
        &FetchPolicyFileLayers {
            system: Some(system_config),
            project: Some(PROJECT_GRANTS),
            ..FetchPolicyFileLayers::default()
        },
        &environment,
        &[
            "netsuke",
            "--fetch-allow-host",
            "cli.example.org",
            "--fetch-block-host",
            "cli-blocked.example.org",
        ],
    )?;

    for allowed in [
        "https://system.example.org",
        "https://environment.example.org",
        "https://cli.example.org",
    ] {
        ensure!(
            merged.network_policy()?.evaluate(&url(allowed)?).is_ok(),
            "trusted allowlist entry should be effective: {allowed}"
        );
    }
    assert_host_not_allowlisted(&merged, "https://169.254.169.254")?;
    assert_scheme_not_allowed(&merged, "http://cli.example.org")?;
    assert_host_blocked(&merged, "https://blocked.example.org")?;
    assert_host_blocked(&merged, "https://environment-blocked.example.org")?;
    assert_host_blocked(&merged, "https://cli-blocked.example.org")
}

#[rstest]
fn project_blocklist_is_cumulative_and_wins_over_operator_allowlist() -> Result<()> {
    let project_config = r#"
fetch_allow_host = ["169.254.169.254"]
fetch_block_host = ["downloads.example.org"]
"#;
    let merged = merge_fetch_policy(
        &FetchPolicyFileLayers {
            project: Some(project_config),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &[
            "netsuke",
            "--fetch-default-deny",
            "--fetch-allow-host",
            "downloads.example.org",
        ],
    )?;

    assert_host_blocked(&merged, "https://downloads.example.org")?;
    assert_host_not_allowlisted(&merged, "https://169.254.169.254")
}

#[rstest]
#[case::system(
    Some("trust_project_fetch_policy = true"),
    None,
    &[],
    &["netsuke"]
)]
#[case::user(
    None,
    Some("trust_project_fetch_policy = true"),
    &[],
    &["netsuke"]
)]
#[case::environment(
    None,
    None,
    &[(OsString::from("NETSUKE_TRUST_PROJECT_FETCH_POLICY"), OsString::from("true"))],
    &["netsuke"]
)]
#[case::cli(
    None,
    None,
    &[],
    &["netsuke", "--trust-project-fetch-policy"]
)]
fn trusted_operator_layers_can_opt_in_to_project_grants(
    #[case] system_config: Option<&str>,
    #[case] user_config: Option<&str>,
    #[case] environment: &[(OsString, OsString)],
    #[case] args: &[&str],
) -> Result<()> {
    let merged = merge_fetch_policy(
        &FetchPolicyFileLayers {
            system: system_config,
            user: user_config,
            project: Some(PROJECT_GRANTS),
        },
        environment,
        args,
    )?;

    ensure!(
        merged.trust_project_fetch_policy,
        "trusted layer should enable opt-in"
    );
    ensure!(
        merged
            .network_policy()?
            .evaluate(&url("http://169.254.169.254")?)
            .is_ok(),
        "opt-in should permit project requested scheme and host"
    );
    Ok(())
}

#[rstest]
fn trusted_project_default_deny_request_can_override_operator_value() -> Result<()> {
    let system_config = r"
fetch_default_deny = true
trust_project_fetch_policy = true
";
    let merged = merge_fetch_policy(
        &FetchPolicyFileLayers {
            system: Some(system_config),
            project: Some(PROJECT_GRANTS),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;

    ensure!(
        !merged.fetch_default_deny,
        "trusted project false request should override operator default-deny"
    );
    ensure!(
        merged
            .network_policy()?
            .evaluate(&url("http://otherwise-unlisted.example.org")?)
            .is_ok(),
        "trusted project request should permit the selected policy"
    );
    Ok(())
}

#[rstest]
fn project_cannot_self_authorize_fetch_policy_widening() -> Result<()> {
    let project_config = format!("trust_project_fetch_policy = true\n{PROJECT_GRANTS}");
    let merged = merge_fetch_policy(
        &FetchPolicyFileLayers {
            project: Some(&project_config),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;

    ensure!(
        !merged.trust_project_fetch_policy,
        "project opt-in request must be removed before generic merging"
    );
    assert_scheme_not_allowed(&merged, "http://169.254.169.254")
}
