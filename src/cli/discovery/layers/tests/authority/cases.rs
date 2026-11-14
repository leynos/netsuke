//! Written cases for definition-authority classification.
//!
//! These drive the classifier directly, including the fail-closed branches that
//! keep an unprovable occurrence from being treated as operator configuration.

use super::super::super::super::paths::{FailingPathNormalizer, FsPathNormalizer};
use super::super::super::super::project_policy::RootAuthority;
use super::super::super::{ProjectKey, ScopedFileLayer, scope_discovered_occurrence};
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use ortho_config::MergeLayer;
use serde_json::json;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use tempfile::{TempDir, tempdir};

/// The operator configuration path every trusted chain in these cases reaches.
const OPERATOR_CONFIG: &str = "/etc/netsuke/config.toml";

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

/// Classify an operator-located occurrence reached through a trusted chain.
fn operator_occurrence(key: Option<&ProjectKey>) -> ScopedFileLayer {
    scope_discovered_occurrence(layer(Some(OPERATOR_CONFIG)), true, key)
}

/// A temporary project root, optionally holding a project configuration file.
struct ProjectFixture {
    root: TempDir,
}

impl ProjectFixture {
    /// Create a project root with no configuration file in it.
    fn empty() -> Result<Self> {
        let root = tempdir().context("create project dir")?;
        Ok(Self { root })
    }

    /// Create a project root and write its project configuration file.
    fn new() -> Result<Self> {
        let fixture = Self::empty()?;
        test_support::fs::write(fixture.file(), "jobs = 3\n").context("write project config")?;
        Ok(fixture)
    }

    /// Borrow the project root.
    fn path(&self) -> &Path {
        self.root.path()
    }

    /// Return the path the project configuration file would occupy.
    fn file(&self) -> PathBuf {
        self.path().join(".netsuke.toml")
    }

    /// Classify the project file with discovery's real normalizer.
    fn key(&self) -> ProjectKey {
        project_key(self.path())
    }

    /// Classify the project file with a normalizer that cannot resolve it.
    fn unresolved_key(&self) -> ProjectKey {
        ProjectKey::classify(&FailingPathNormalizer, &self.file())
    }
}

#[test]
fn trusted_root_keeps_operator_authority() -> Result<()> {
    let project = ProjectFixture::new()?;

    let occurrence = operator_occurrence(Some(&project.key()));

    ensure!(
        occurrence.authority() == RootAuthority::Operator,
        "an operator-located occurrence should keep operator authority"
    );
    Ok(())
}

#[test]
fn project_occurrence_inside_trusted_chain_is_untrusted() -> Result<()> {
    let project = ProjectFixture::new()?;
    let recorded = canonical(&project.file())?;

    let occurrence =
        scope_discovered_occurrence(layer(Some(&recorded)), true, Some(&project.key()));

    ensure!(
        occurrence.authority() == RootAuthority::Untrusted,
        "the project file reached from an operator root must not grant authority"
    );
    Ok(())
}

#[test]
fn untrusted_root_withholds_authority_from_every_occurrence() -> Result<()> {
    let occurrence = scope_discovered_occurrence(layer(Some(OPERATOR_CONFIG)), false, None);

    ensure!(
        occurrence.authority() == RootAuthority::Untrusted,
        "a chain descending from an untrusted root grants no authority"
    );
    Ok(())
}

#[test]
fn unresolved_project_key_withholds_operator_authority() -> Result<()> {
    let project = ProjectFixture::new()?;

    let occurrence = operator_occurrence(Some(&project.unresolved_key()));

    ensure!(
        occurrence.authority() == RootAuthority::Untrusted,
        "an existing project file that cannot be normalized must fail closed"
    );
    Ok(())
}

#[test]
fn absent_project_file_keeps_operator_authority() -> Result<()> {
    let project = ProjectFixture::empty()?;

    let occurrence = operator_occurrence(Some(&project.unresolved_key()));

    ensure!(
        occurrence.authority() == RootAuthority::Operator,
        "no project file means no project occurrence to distrust"
    );
    Ok(())
}

#[test]
fn occurrence_without_a_path_is_untrusted() -> Result<()> {
    let project = ProjectFixture::new()?;

    let occurrence = scope_discovered_occurrence(layer(None), true, Some(&project.key()));

    ensure!(
        occurrence.authority() == RootAuthority::Untrusted,
        "an occurrence with unknown provenance must not be presumed operator-owned"
    );
    Ok(())
}
