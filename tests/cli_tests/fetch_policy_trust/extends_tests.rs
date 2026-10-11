//! Regress project-chain provenance through real CLI configuration merging.

use super::*;
use anyhow::{Result, ensure};
use netsuke::host_pattern::HostPattern;
use rstest::rstest;

#[rstest]
fn inherited_project_grants_cannot_widen_user_policy() -> Result<()> {
    let merged = merge_fetch_policy(
        &FetchPolicyFileLayers {
            user: Some("fetch_default_deny = true\n"),
            project: Some("extends = \"policy.toml\"\n"),
            project_files: Some(vec![(
                "policy.toml",
                "fetch_allow_scheme = [\"http\"]\nfetch_allow_host = [\"169.254.169.254\"]\n",
            )]),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;

    assert_scheme_not_allowed(&merged, "http://169.254.169.254")?;
    assert_host_not_allowlisted(&merged, "https://169.254.169.254")
}

#[rstest]
fn inherited_project_default_deny_cannot_weaken_user_policy() -> Result<()> {
    let merged = merge_fetch_policy(
        &FetchPolicyFileLayers {
            user: Some("fetch_default_deny = true\nfetch_allow_host = [\"operator.example\"]\n"),
            project: Some("extends = \"policy.toml\"\n"),
            project_files: Some(vec![("policy.toml", "fetch_default_deny = false\n")]),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;

    ensure!(merged.fetch_default_deny, "inherited false must be ignored");
    assert_host_not_allowlisted(&merged, "https://otherwise-unlisted.example")
}

#[rstest]
fn inherited_trust_cannot_authorize_primary_or_inherited_grants() -> Result<()> {
    let merged = merge_fetch_policy(
        &FetchPolicyFileLayers {
            user: Some("fetch_default_deny = true\n"),
            project: Some(
                "extends = \"policy.toml\"\nfetch_allow_scheme = [\"http\"]\nfetch_allow_host = [\"primary.example\"]\n",
            ),
            project_files: Some(vec![(
                "policy.toml",
                "trust_project_fetch_policy = true\nfetch_allow_scheme = [\"http\"]\nfetch_allow_host = [\"inherited.example\"]\n",
            )]),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;

    ensure!(
        !merged.trust_project_fetch_policy,
        "inherited trust request must not become operator policy"
    );
    assert_scheme_not_allowed(&merged, "http://primary.example")?;
    assert_host_not_allowlisted(&merged, "https://primary.example")?;
    assert_scheme_not_allowed(&merged, "http://inherited.example")?;
    assert_host_not_allowlisted(&merged, "https://inherited.example")
}

#[rstest]
fn inherited_project_blocks_remain_cumulative_and_authoritative() -> Result<()> {
    let merged = merge_fetch_policy(
        &FetchPolicyFileLayers {
            user: Some(concat!(
                "fetch_default_deny = true\n",
                "fetch_allow_scheme = [\"https\"]\n",
                "fetch_allow_host = [\"operator-blocked.example\", ",
                "\"inherited-blocked.example\", \"primary-blocked.example\"]\n",
                "fetch_block_host = [\"operator-blocked.example\"]\n",
            )),
            project: Some(
                "extends = \"policy.toml\"\nfetch_block_host = [\"primary-blocked.example\"]\n",
            ),
            project_files: Some(vec![(
                "policy.toml",
                "fetch_block_host = [\"inherited-blocked.example\"]\n",
            )]),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;

    for blocked in [
        "https://operator-blocked.example",
        "https://inherited-blocked.example",
        "https://primary-blocked.example",
    ] {
        assert_host_blocked(&merged, blocked)?;
    }
    Ok(())
}

#[rstest]
fn trusted_user_policy_accepts_the_project_chain_in_dependency_order() -> Result<()> {
    let merged = merge_fetch_policy(
        &FetchPolicyFileLayers {
            user: Some(concat!(
                "trust_project_fetch_policy = true\n",
                "fetch_default_deny = true\n",
                "fetch_allow_scheme = [\"http\"]\n",
                "fetch_allow_host = [\"operator.example\"]\n",
            )),
            project: Some(
                "extends = \"middle.toml\"\nfetch_allow_scheme = [\"http\"]\nfetch_allow_host = [\"primary.example\"]\n",
            ),
            project_files: Some(vec![
                (
                    "base.toml",
                    concat!(
                        "fetch_default_deny = true\n",
                        "fetch_allow_scheme = [\"ftp\"]\n",
                        "fetch_allow_host = [\"base.example\"]\n",
                    ),
                ),
                (
                    "middle.toml",
                    concat!(
                        "extends = \"base.toml\"\n",
                        "fetch_default_deny = false\n",
                        "fetch_allow_scheme = [\"https\"]\n",
                        "fetch_allow_host = [\"middle.example\"]\n",
                    ),
                ),
            ]),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;

    ensure!(
        !merged.fetch_default_deny,
        "the last present project value from the middle layer must apply"
    );
    ensure!(
        merged.fetch_allow_scheme == ["http", "ftp", "https", "http"].map(String::from),
        "accepted schemes must retain operator then dependency-first order: {:?}",
        merged.fetch_allow_scheme
    );
    let expected_hosts = [
        "operator.example",
        "base.example",
        "middle.example",
        "primary.example",
    ]
    .map(HostPattern::parse)
    .into_iter()
    .collect::<std::result::Result<Vec<_>, _>>()?;
    ensure!(
        merged.fetch_allow_host == expected_hosts,
        "accepted hosts must retain operator then dependency-first order"
    );
    for value in [
        "http://operator.example",
        "ftp://base.example",
        "https://middle.example",
        "http://primary.example",
    ] {
        ensure!(
            merged.network_policy()?.evaluate(&url(value)?).is_ok(),
            "trusted project-chain grant should remain effective: {value}"
        );
    }
    Ok(())
}

#[rstest]
fn user_configuration_extends_retains_operator_grants() -> Result<()> {
    let merged = merge_fetch_policy(
        &FetchPolicyFileLayers {
            user: Some(
                "extends = \"operator-policy.toml\"\nfetch_default_deny = true\nfetch_allow_scheme = [\"http\"]\n",
            ),
            user_files: Some(vec![(
                "operator-policy.toml",
                "fetch_allow_host = [\"operator-parent.example\"]\n",
            )]),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;

    ensure!(
        merged
            .network_policy()?
            .evaluate(&url("http://operator-parent.example")?)
            .is_ok(),
        "a user-root parent retains operator authority"
    );
    Ok(())
}

#[rstest]
fn shared_file_keeps_operator_authority_only_for_its_operator_occurrence() -> Result<()> {
    let shared_policy = "fetch_allow_host = [\"shared.example\"]\n";
    let shared_from_both_roots = merge_fetch_policy(
        &FetchPolicyFileLayers {
            user: Some(
                "extends = \"{project_dir}/shared.toml\"\nfetch_default_deny = true\nfetch_allow_scheme = [\"http\"]\n",
            ),
            project: Some("extends = \"{project_dir}/shared.toml\"\n"),
            project_files: Some(vec![("shared.toml", shared_policy)]),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;
    ensure!(
        shared_from_both_roots
            .network_policy()?
            .evaluate(&url("http://shared.example")?)
            .is_ok(),
        "the same file's operator-root occurrence must retain its grant"
    );

    let project_only_occurrence = merge_fetch_policy(
        &FetchPolicyFileLayers {
            user: Some("fetch_default_deny = true\nfetch_allow_scheme = [\"http\"]\n"),
            project: Some("extends = \"{project_dir}/shared.toml\"\n"),
            project_files: Some(vec![("shared.toml", shared_policy)]),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;
    assert_host_not_allowlisted(&project_only_occurrence, "http://shared.example")
}

#[rstest]
fn three_level_project_chain_grants_remain_quarantined() -> Result<()> {
    let merged = merge_fetch_policy(
        &FetchPolicyFileLayers {
            user: Some("fetch_default_deny = true\n"),
            project: Some(
                "extends = \"middle.toml\"\nfetch_allow_scheme = [\"http\"]\nfetch_allow_host = [\"primary.example\"]\n",
            ),
            project_files: Some(vec![
                (
                    "base.toml",
                    "fetch_allow_scheme = [\"http\"]\nfetch_allow_host = [\"base.example\"]\n",
                ),
                (
                    "middle.toml",
                    "extends = \"base.toml\"\nfetch_allow_scheme = [\"http\"]\nfetch_allow_host = [\"middle.example\"]\n",
                ),
            ]),
            ..FetchPolicyFileLayers::default()
        },
        &[],
        &["netsuke"],
    )?;

    for host in ["base.example", "middle.example", "primary.example"] {
        assert_scheme_not_allowed(&merged, &format!("http://{host}"))?;
        assert_host_not_allowlisted(&merged, &format!("https://{host}"))?;
    }
    ensure!(
        merged.fetch_default_deny,
        "user default-deny must remain active"
    );
    Ok(())
}
