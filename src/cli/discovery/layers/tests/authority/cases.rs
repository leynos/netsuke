//! Written cases for definition-authority classification.
//!
//! These drive the classifier directly, including the fail-closed branches that
//! keep an unprovable occurrence from being treated as operator configuration.

use super::super::super::super::paths::{
    FailingPathNormalizer, FsPathNormalizer, UnreadablePathNormalizer,
};
use super::super::super::super::project_policy::RootAuthority;
use super::super::super::{
    CandidateIdentity, ProjectKey, ScopedFileLayer, scope_discovered_occurrence,
};
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

    /// Classify the project file with a normalizer that cannot probe existence.
    fn unreadable_key(&self) -> ProjectKey {
        ProjectKey::classify(&UnreadablePathNormalizer, &self.file())
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
fn unreadable_existence_withholds_operator_authority() -> Result<()> {
    let project = ProjectFixture::new()?;

    let occurrence = operator_occurrence(Some(&project.unreadable_key()));

    ensure!(
        occurrence.authority() == RootAuthority::Untrusted,
        "a project file whose existence cannot be probed must fail closed"
    );
    Ok(())
}

#[test]
fn unknown_existence_is_not_read_as_absence() -> Result<()> {
    let project = ProjectFixture::empty()?;

    let occurrence = operator_occurrence(Some(&project.unreadable_key()));

    ensure!(
        occurrence.authority() == RootAuthority::Untrusted,
        "a failed existence query must not license operator authority"
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

/// Classify a project file that exists under a directory name Unix cannot spell
/// as UTF-8.
///
/// Normalization has to read the path the caller held, because its lossy
/// spelling names no file on disk: resolving that spelling fails, and the
/// failure marks the key ambiguous, which withholds operator authority from
/// every occurrence in a workspace that is perfectly valid. Unix is the only
/// supported platform where a directory name can be unspellable text.
#[cfg(unix)]
#[test]
fn non_utf8_project_root_still_resolves() -> Result<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let root = tempdir().context("create project dir")?;
    let directory = root
        .path()
        .join(OsString::from_vec(b"project-\xff".to_vec()));
    test_support::fs::create_dir_all(&directory).context("create non-UTF-8 project dir")?;
    let file = directory.join(".netsuke.toml");
    test_support::fs::write(&file, "jobs = 3\n").context("write project config")?;

    let key = project_key(&directory);

    ensure!(
        !key.is_ambiguous(),
        "an existing project file under a non-UTF-8 directory must resolve"
    );
    ensure!(
        key.matches_path(&dunce::canonicalize(&file).context("canonicalize project file")?),
        "the key must recognize the recorded spelling of the file it classified"
    );
    Ok(())
}

/// A candidate that resolves to the project file is identified as such.
#[test]
fn resolved_project_candidate_is_identified() -> Result<()> {
    let project = ProjectFixture::new()?;

    let identity = project
        .key()
        .classify_candidate(&FsPathNormalizer, &project.file());

    ensure!(
        identity == CandidateIdentity::Project,
        "the project file's own path must be identified, got {identity:?}"
    );
    Ok(())
}

/// A candidate that resolves elsewhere is provably not the project file.
#[test]
fn resolved_other_candidate_is_not_the_project_file() -> Result<()> {
    let project = ProjectFixture::new()?;
    let other = write_other_config(&project)?;

    let identity = project.key().classify_candidate(&FsPathNormalizer, &other);

    ensure!(
        identity == CandidateIdentity::Other,
        "a resolved path that differs from the key must be identified as other, got {identity:?}"
    );
    Ok(())
}

/// A literal spelling still identifies the project file when resolution fails.
///
/// This is the one resolution failure that is evidence rather than doubt: the
/// candidate's own text names the project path, so treating it as unresolved
/// would grant authority to a candidate that is provably project-controlled.
#[test]
fn unresolvable_candidate_literally_matching_the_key_is_identified() -> Result<()> {
    let project = ProjectFixture::new()?;
    let literal = PathBuf::from(canonical(&project.file())?);

    let identity = project
        .key()
        .classify_candidate(&FailingPathNormalizer, &literal);

    ensure!(
        identity == CandidateIdentity::Project,
        "an exactly-spelled path must stay identified when it cannot resolve, got {identity:?}"
    );
    Ok(())
}

/// A candidate that neither resolves nor matches literally is unresolved.
///
/// Reporting it as a plain miss would let an identity that cannot be
/// established donate operator authority, so the state has to be distinct.
#[test]
fn unresolvable_candidate_with_another_spelling_is_unresolved() -> Result<()> {
    let project = ProjectFixture::new()?;
    let literal = PathBuf::from(canonical(&write_other_config(&project)?)?);

    let identity = project
        .key()
        .classify_candidate(&FailingPathNormalizer, &literal);

    ensure!(
        identity == CandidateIdentity::Unresolved,
        "a candidate that cannot be ruled out as the project file must report so, got {identity:?}"
    );
    Ok(())
}

/// Write a configuration file that is not the project configuration.
fn write_other_config(project: &ProjectFixture) -> Result<PathBuf> {
    let other = project.path().join("other.toml");
    test_support::fs::write(&other, "jobs = 5\n").context("write the other config")?;
    Ok(other)
}
