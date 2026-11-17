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
            entry_result
                .and_then(|dir_entry| dir_entry.file_name().map(|name| entry_display_path(&name)))
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

/// Describe a run of sidecars that differ only in their sequence number.
///
/// The count belongs with the naming scheme because together they define the
/// series; keeping them in one value stops the publishing helper's parameter
/// list from growing one argument per naming detail.
#[derive(Clone, Copy)]
struct SidecarSeries<'a> {
    prefix: &'a str,
    content: &'a str,
    publications: usize,
}

impl<'a> SidecarSeries<'a> {
    /// Name a series that publishes `content` `publications` times.
    const fn new(prefix: &'a str, content: &'a str, publications: usize) -> Self {
        Self {
            prefix,
            content,
            publications,
        }
    }
}

/// Publish `series` repeatedly under `policy` and return the final bundle.
///
/// Every publication prunes under the same policy, so the loop also checks the
/// invariant every caller shares: the bundle published last is still readable
/// afterwards. Callers then measure the single bound they are responsible for.
fn publish_repeated_sidecars(
    dir: &Dir,
    series: SidecarSeries<'_>,
    policy: RetentionPolicy,
) -> Result<GeneratedDyndep> {
    let mut latest_sidecar = None;

    for index in 0..series.publications {
        let path = format!(".netsuke/dyndep/{}-{index}.dd", series.prefix);
        let current = sidecar(&path, series.content);
        let lease = materialize_dyndep_files(dir, std::slice::from_ref(&current))?;
        prune_dyndep_sidecars(dir, &lease, std::slice::from_ref(&current), policy)?;
        latest_sidecar = Some(current);
        drop(lease);
    }

    let latest = latest_sidecar.context("loop must publish a current sidecar")?;
    ensure!(
        dir.open(latest.relative_path()).is_ok(),
        "retention must preserve the latest bundle's sidecar"
    );
    Ok(latest)
}

mod budget;

mod entries;

mod lease_tests;

mod retention_proptests;

mod telemetry_tests;
