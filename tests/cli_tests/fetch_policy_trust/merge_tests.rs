//! Existing operator and primary-project fetch-policy merge regressions.

use super::*;
use anyhow::{Result, ensure};
use rstest::rstest;
use std::ffi::OsString;

const PROJECT_GRANTS: &str = r#"
fetch_default_deny = false
fetch_allow_scheme = ["http"]
fetch_allow_host = ["169.254.169.254"]
"#;

const PROJECT_TIGHTENING: &str = "fetch_default_deny = true";

#[rstest]
fn defaults_keep_project_fetch_grants_outside_operator_policy() -> Result<()> {
    let merged = merge_project_fetch_policy(PROJECT_GRANTS)?;

    ensure!(
        !merged.fetch_default_deny,
        "a project false request should not alter the default operator policy"
    );
    assert_scheme_not_allowed(&merged, "http://169.254.169.254")
}

#[rstest]
fn project_can_tighten_default_deny_without_operator_policy() -> Result<()> {
    let merged = merge_project_fetch_policy(PROJECT_TIGHTENING)?;

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
            ..FetchPolicyFileLayers::default()
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
