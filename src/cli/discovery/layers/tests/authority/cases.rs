//! Written cases for definition-authority classification.
//!
//! These drive the classifier directly, including the fail-closed branches that
//! keep an unprovable occurrence from being treated as operator configuration.

use super::super::super::super::paths::{FailingPathNormalizer, FsPathNormalizer};
use super::super::super::super::project_policy::RootAuthority;
use super::super::super::{ProjectKey, scope_discovered_occurrence};
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use ortho_config::MergeLayer;
use serde_json::json;
use std::borrow::Cow;
use std::path::Path;
use tempfile::tempdir;

/// Build a layer with an optional recorded path.
fn layer(path: Option<&str>) -> MergeLayer<'static> {
    MergeLayer::file(Cow::Owned(json!({"jobs": 2})), path.map(Utf8PathBuf::from))
}

/// Classify the project file at `directory/.netsuke.toml` with `normalizer`.
fn project_key(directory: &Path) -> ProjectKey {
    ProjectKey::classify(&FsPathNormalizer, &directory.join(".netsuke.toml"))
}

/// Return `path` in the canonical form discovery records for layers.
fn canonical(path: &Path) -> Result<String> {
    let canonical = dunce::canonicalize(path).context("canonicalize test path")?;
    canonical
        .to_str()
        .map(ToOwned::to_owned)
        .context("canonical test path is not valid UTF-8")
}

#[test]
fn trusted_root_keeps_operator_authority() -> Result<()> {
    let project_dir = tempdir().context("create project dir")?;
    test_support::fs::write(project_dir.path().join(".netsuke.toml"), "jobs = 3\n")
        .context("write project config")?;
    let key = project_key(project_dir.path());

    let occurrence =
        scope_discovered_occurrence(layer(Some("/etc/netsuke/config.toml")), true, Some(&key));

    ensure!(
        occurrence.authority() == RootAuthority::Operator,
        "an operator-located occurrence should keep operator authority"
    );
    Ok(())
}

#[test]
fn project_occurrence_inside_trusted_chain_is_untrusted() -> Result<()> {
    let project_dir = tempdir().context("create project dir")?;
    let project_file = project_dir.path().join(".netsuke.toml");
    test_support::fs::write(&project_file, "jobs = 3\n").context("write project config")?;
    let key = project_key(project_dir.path());
    let recorded = canonical(&project_file)?;

    let occurrence = scope_discovered_occurrence(layer(Some(&recorded)), true, Some(&key));

    ensure!(
        occurrence.authority() == RootAuthority::Untrusted,
        "the project file reached from an operator root must not grant authority"
    );
    Ok(())
}

#[test]
fn untrusted_root_withholds_authority_from_every_occurrence() -> Result<()> {
    let occurrence =
        scope_discovered_occurrence(layer(Some("/etc/netsuke/config.toml")), false, None);

    ensure!(
        occurrence.authority() == RootAuthority::Untrusted,
        "a chain descending from an untrusted root grants no authority"
    );
    Ok(())
}

#[test]
fn unresolved_project_key_withholds_operator_authority() -> Result<()> {
    let project_dir = tempdir().context("create project dir")?;
    test_support::fs::write(project_dir.path().join(".netsuke.toml"), "jobs = 3\n")
        .context("write project config")?;
    let key = ProjectKey::classify(
        &FailingPathNormalizer,
        &project_dir.path().join(".netsuke.toml"),
    );

    let occurrence =
        scope_discovered_occurrence(layer(Some("/etc/netsuke/config.toml")), true, Some(&key));

    ensure!(
        occurrence.authority() == RootAuthority::Untrusted,
        "an existing project file that cannot be normalized must fail closed"
    );
    Ok(())
}

#[test]
fn absent_project_file_keeps_operator_authority() -> Result<()> {
    let project_dir = tempdir().context("create project dir")?;
    let key = ProjectKey::classify(
        &FailingPathNormalizer,
        &project_dir.path().join(".netsuke.toml"),
    );

    let occurrence =
        scope_discovered_occurrence(layer(Some("/etc/netsuke/config.toml")), true, Some(&key));

    ensure!(
        occurrence.authority() == RootAuthority::Operator,
        "no project file means no project occurrence to distrust"
    );
    Ok(())
}

#[test]
fn occurrence_without_a_path_is_untrusted() -> Result<()> {
    let project_dir = tempdir().context("create project dir")?;
    test_support::fs::write(project_dir.path().join(".netsuke.toml"), "jobs = 3\n")
        .context("write project config")?;
    let key = project_key(project_dir.path());

    let occurrence = scope_discovered_occurrence(layer(None), true, Some(&key));

    ensure!(
        occurrence.authority() == RootAuthority::Untrusted,
        "an occurrence with unknown provenance must not be presumed operator-owned"
    );
    Ok(())
}
