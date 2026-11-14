//! Focused tests for bounded dyndep sidecar retention.

use super::super::super::tests::temporary_dir;
use super::*;
use crate::ninja_gen::GeneratedDyndep;
use crate::runner::process::dyndep::telemetry::{
    RETAINED_BYTES_RECLAIMED, RETAINED_FILES_RECLAIMED, RETENTIONS_TOTAL,
};
use crate::runner::process::materialize_dyndep_files;
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;
use rstest::{fixture, rstest};

#[fixture]
fn dyndep_workspace() -> Result<(tempfile::TempDir, Dir)> {
    let workspace = tempfile::tempdir()?;
    let dir = temporary_dir(&workspace)?;
    Ok((workspace, dir))
}

struct PublishedCurrentSidecar {
    _workspace: tempfile::TempDir,
    dir: Dir,
    current: GeneratedDyndep,
    lease: DyndepPublicationLease,
}

#[fixture]
fn published_current_sidecar(
    dyndep_workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<PublishedCurrentSidecar> {
    let (workspace, dir) = dyndep_workspace?;
    let current = sidecar(".netsuke/dyndep/current.dd", "current");
    let lease = materialize_dyndep_files(&dir, std::slice::from_ref(&current))?;
    Ok(PublishedCurrentSidecar {
        _workspace: workspace,
        dir,
        current,
        lease,
    })
}

fn sidecar(name: &str, content: &str) -> GeneratedDyndep {
    GeneratedDyndep::fixture(Utf8PathBuf::from(name), content.to_owned())
}

fn sidecar_names(dir: &Dir) -> Result<Vec<Utf8PathBuf>> {
    dir.read_dir(DYNDEP_DIR)?
        .map(|entry_result| {
            entry_result.and_then(|dir_entry| {
                dir_entry
                    .file_name()
                    .map(|name| Utf8Path::new(DYNDEP_DIR).join(name))
            })
        })
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn retained_sidecar_bytes(dir: &Dir) -> Result<u64> {
    sidecar_names(dir)?
        .into_iter()
        .filter(|path| has_extension(path, "dd"))
        .try_fold(0_u64, |total, path| {
            Ok(total.saturating_add(dir.metadata(path)?.len()))
        })
}

fn publish_repeated_sidecars(
    dir: &Dir,
    prefix: &str,
    content: &str,
    policy: RetentionPolicy,
) -> Result<GeneratedDyndep> {
    let mut latest_sidecar = None;

    for index in 0..6 {
        let path = format!(".netsuke/dyndep/{prefix}-{index}.dd");
        let current = sidecar(&path, content);
        let lease = materialize_dyndep_files(dir, std::slice::from_ref(&current))?;
        prune_dyndep_sidecars(dir, &lease, std::slice::from_ref(&current), policy)?;
        latest_sidecar = Some(current);
        drop(lease);
    }

    latest_sidecar.context("loop must publish a current sidecar")
}

mod budget;

mod entries;

mod lease_tests;

mod telemetry_tests;
