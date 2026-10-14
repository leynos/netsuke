//! Verify project-chain quarantine and per-occurrence configuration authority.

use super::super::super::discover_file_layers;
use super::super::{retain_layers_and_resolve_json, scope_merged_layers};
use super::{TestEnv, utf8_directory};
use crate::cli::Cli;
use crate::host_pattern::HostPattern;
use anyhow::{Context, Result, ensure};
use ortho_config::MergeLayer;
use serde_json::json;
use std::borrow::Cow;
use std::path::Path;
use tempfile::tempdir;

/// Discover a project chain and return its quarantined request and generic layers.
fn discover_project_chain(
    directory: &Path,
    inherited_layers: &[(&str, &str)],
    primary_contents: &str,
) -> Result<(
    Option<super::super::super::ProjectFetchPolicyRequest>,
    Vec<MergeLayer<'static>>,
)> {
    for (relative_path, contents) in inherited_layers {
        test_support::fs::write(directory.join(relative_path), contents)
            .with_context(|| format!("write inherited project layer {relative_path}"))?;
    }
    test_support::fs::write(directory.join(".netsuke.toml"), primary_contents)
        .context("write primary project layer")?;
    let cli = Cli {
        directory: Some(utf8_directory(directory)?),
        ..Cli::default()
    };
    let discovered = discover_file_layers(&cli, &TestEnv::default());
    let (layers, errors, request, _) = discovered.into_layers().into_parts();
    ensure!(errors.is_empty(), "project chain should load: {errors:?}");
    Ok((request, layers))
}

#[test]
fn two_level_project_chain_quarantines_inherited_fetch_grants_and_trust() -> Result<()> {
    let temp = tempdir().context("create project directory")?;
    let (project_request, layers) = discover_project_chain(
        temp.path(),
        &[(
            "policy.toml",
            concat!(
                "fetch_default_deny = false\n",
                "fetch_allow_scheme = [\"http\"]\n",
                "fetch_allow_host = [\"169.254.169.254\"]\n",
                "trust_project_fetch_policy = true\n",
                "env_allow_var = [\"CI_SECRET\"]\n",
                "fetch_block_host = [\"blocked.example\"]\n",
                "env_block_var = [\"BLOCKED_SECRET\"]\n",
            ),
        )],
        "extends = \"policy.toml\"\nfetch_allow_host = [\"primary.example\"]\n",
    )?;

    let request = project_request.context("project chain should produce a request")?;
    ensure!(
        request.default_deny == Some(false),
        "inherited default is captured"
    );
    ensure!(
        request.allow_scheme == ["http"],
        "inherited scheme is captured"
    );
    ensure!(
        request.allow_host
            == [
                HostPattern::parse("169.254.169.254")?,
                HostPattern::parse("primary.example")?,
            ],
        "dependency and primary hosts should be appended in order"
    );
    ensure!(layers.len() == 2, "both project occurrences should remain");
    for layer in layers {
        let value = layer.into_value();
        for field in [
            "fetch_default_deny",
            "fetch_allow_scheme",
            "fetch_allow_host",
            "trust_project_fetch_policy",
            "env_allow_var",
        ] {
            ensure!(value.get(field).is_none(), "{field} must be quarantined");
        }
    }
    Ok(())
}

#[test]
fn three_level_project_chain_accumulates_grants_in_dependency_order() -> Result<()> {
    let temp = tempdir().context("create project directory")?;
    let (project_request, layers) = discover_project_chain(
        temp.path(),
        &[
            (
                "base.toml",
                "fetch_default_deny = true\nfetch_allow_scheme = [\"base\"]\nfetch_allow_host = [\"base.example\"]\n",
            ),
            (
                "middle.toml",
                "extends = \"base.toml\"\nfetch_default_deny = false\nfetch_allow_scheme = [\"middle\"]\nfetch_allow_host = [\"middle.example\"]\n",
            ),
        ],
        "extends = \"middle.toml\"\nfetch_allow_scheme = [\"primary\"]\nfetch_allow_host = [\"primary.example\"]\n",
    )?;
    let request = project_request.context("three-level project chain should produce a request")?;

    ensure!(
        request.allow_scheme == ["base", "middle", "primary"],
        "schemes should append in dependency-first order"
    );
    ensure!(
        request.allow_host
            == [
                HostPattern::parse("base.example")?,
                HostPattern::parse("middle.example")?,
                HostPattern::parse("primary.example")?,
            ],
        "hosts should append in dependency-first order"
    );
    ensure!(
        request.default_deny == Some(false),
        "last present default-deny value should win"
    );
    ensure!(layers.len() == 3, "all project occurrences should remain");
    Ok(())
}

#[test]
fn overlapping_roots_keep_operator_and_project_occurrences_separate() -> Result<()> {
    let shared_path = "shared-policy.toml".to_owned();
    let operator = MergeLayer::file(
        Cow::Owned(json!({"fetch_allow_host": ["operator.example"]})),
        Some(shared_path.clone().into()),
    );
    let project_shared = MergeLayer::file(
        Cow::Owned(json!({"fetch_allow_host": ["project.example"]})),
        Some(shared_path.into()),
    );
    let project_primary = MergeLayer::file(
        Cow::Owned(json!({"fetch_default_deny": true})),
        Some(".netsuke.toml".into()),
    );

    let resolved = retain_layers_and_resolve_json(scope_merged_layers(
        vec![operator],
        vec![project_shared, project_primary],
        None,
    ));

    ensure!(
        resolved
            .project_request
            .as_ref()
            .map(|request| request.default_deny)
            == Some(Some(true)),
        "project occurrence should retain its request"
    );
    ensure!(
        resolved
            .project_request
            .as_ref()
            .map(|request| request.allow_host.clone())
            == Some(vec![HostPattern::parse("project.example")?]),
        "project occurrence should be quarantined independently"
    );
    let mut layers = resolved.layers.into_iter();
    let operator_value = layers
        .next()
        .context("operator occurrence should be retained")?
        .into_value();
    ensure!(
        operator_value.get("fetch_allow_host") == Some(&json!(["operator.example"])),
        "operator occurrence must preserve its host grant"
    );
    let project_value = layers
        .next()
        .context("project occurrence should be retained")?
        .into_value();
    ensure!(
        project_value.get("fetch_allow_host").is_none(),
        "project occurrence must quarantine the shared file grant"
    );
    Ok(())
}
