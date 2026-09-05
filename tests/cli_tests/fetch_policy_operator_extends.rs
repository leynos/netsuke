//! Verify project provenance when an operator root includes the primary file.

use super::{ChainFixture, OPERATOR_POLICY, PROJECT_GRANTS, chain_fixture, evaluate};
use anyhow::{Context, Result, ensure};
use netsuke::stdlib::NetworkPolicyViolation;
use rstest::rstest;

#[rstest]
#[case::automatic_user(false)]
#[case::explicit_operator(true)]
fn operator_including_primary_preserves_project_quarantine(
    chain_fixture: Result<ChainFixture>,
    #[case] explicit: bool,
) -> Result<()> {
    let fixture = chain_fixture?;
    let hostile_policy = format!("trust_project_fetch_policy = true\n{PROJECT_GRANTS}");
    fixture.write_chain(&hostile_policy, &hostile_policy, true)?;
    fixture.write_user_extending(
        &fixture.project.path().join(".netsuke.toml"),
        OPERATOR_POLICY,
    )?;
    let operator_path = fixture.home.path().join(".config/netsuke/config.toml");
    let operator_path_text = operator_path.to_str().context("UTF-8 operator path")?;
    let args = if explicit {
        vec!["netsuke", "--config", operator_path_text]
    } else {
        vec!["netsuke"]
    };
    let merged = fixture.merge(&[], &args)?;
    ensure!(!merged.trust_project_fetch_policy);
    ensure!(merged.fetch_default_deny);
    ensure!(evaluate(&merged, "https://downloads.example.org")?.is_ok());
    ensure!(matches!(
        evaluate(&merged, "https://169.254.169.254")?,
        Err(NetworkPolicyViolation::HostNotAllowlisted { .. })
    ));
    ensure!(matches!(
        evaluate(&merged, "http://downloads.example.org")?,
        Err(NetworkPolicyViolation::SchemeNotAllowed { .. })
    ));
    Ok(())
}
