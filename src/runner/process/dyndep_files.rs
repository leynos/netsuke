//! Atomic materialization of generated Ninja dyndep sidecars.
//!
//! Serial-dependency manifests reference dyndep sidecars beneath
//! `.netsuke/dyndep` in the effective Ninja working directory. Ninja requires
//! those files to exist before it loads a serial build file, so every runner
//! path materializes them before writing or invoking the main file. Sidecar
//! filenames are content-addressed, so writes are deterministic and idempotent:
//! an existing file whose bytes match is reused, while a mismatch is treated as
//! corruption.
//!
//! All writes go through a capability-scoped directory handle opened on the
//! effective Ninja working directory, using a same-directory temporary file and
//! an atomic rename so concurrent Netsuke processes cannot observe partial
//! content.

use crate::cli::Cli;
use crate::localization::{self, keys};
use crate::ninja_gen::GeneratedDyndep;
use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, OpenOptions};
use std::io::{Read, Write};

/// Namespace for generated dyndep sidecar files.
pub(crate) const DYNDEP_DIR: &str = ".netsuke/dyndep";

/// Materialize every sidecar in `dyndep_files` under the effective Ninja
/// working directory selected by `cli`.
///
/// # Errors
///
/// Returns an error if the working directory cannot be opened or the
/// `.netsuke/dyndep` directory created, or if any sidecar write, rename, or
/// content verification fails.
pub fn materialize_dyndep_files(cli: &Cli, dyndep_files: &[GeneratedDyndep]) -> Result<()> {
    let dir = open_effective_dir(cli)?;
    dir.create_dir_all(DYNDEP_DIR).with_context(|| {
        localization::message(keys::RUNNER_IO_DYNDEP_CREATE_DIR)
            .with_arg("path", DYNDEP_DIR.to_owned())
    })?;
    for sidecar in dyndep_files {
        materialize_one(&dir, sidecar)?;
    }
    Ok(())
}

/// Open the effective Ninja working directory through the capability seam.
///
/// Honours the CLI `--directory` option; otherwise uses the current directory.
fn open_effective_dir(cli: &Cli) -> Result<Dir> {
    if let Some(dir) = &cli.directory {
        let utf8 = Utf8Path::from_path(dir).context("non-UTF-8 working directory")?;
        Dir::open_ambient_dir(utf8.as_str(), ambient_authority()).with_context(|| {
            localization::message(keys::RUNNER_IO_OPEN_AMBIENT_DIR).with_arg("path", utf8.as_str())
        })
    } else {
        Dir::open_ambient_dir(".", ambient_authority())
            .context(localization::message(keys::RUNNER_IO_OPEN_AMBIENT_DIR))
    }
}

/// Materialize one sidecar idempotently and atomically.
fn materialize_one(dir: &Dir, sidecar: &GeneratedDyndep) -> Result<()> {
    let rel = sidecar.relative_path().clone();
    match read_verified(dir, &rel, sidecar.content())? {
        ReadOutcome::Matching => {
            tracing::debug!(
                path = %rel,
                "reusing existing dyndep sidecar",
            );
            Ok(())
        }
        ReadOutcome::Mismatch => Err(anyhow!(
            localization::message(keys::RUNNER_IO_DYNDEP_CORRUPT).with_arg("path", rel.as_str())
        )),
        ReadOutcome::Missing => write_atomic(dir, &rel, sidecar.content()),
    }
}

#[derive(PartialEq)]
enum ReadOutcome {
    Matching,
    Mismatch,
    Missing,
}

/// Read an existing sidecar and compare it with the expected content.
fn read_verified(dir: &Dir, rel: &Utf8Path, expected: &str) -> Result<ReadOutcome> {
    let mut options = OpenOptions::new();
    options.read(true);
    let mut file = match dir.open_with(rel, &options) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(ReadOutcome::Missing),
        Err(err) => {
            return Err(err).with_context(|| {
                localization::message(keys::RUNNER_IO_DYNDEP_READ).with_arg("path", rel.as_str())
            });
        }
    };
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).with_context(|| {
        localization::message(keys::RUNNER_IO_DYNDEP_READ).with_arg("path", rel.as_str())
    })?;
    if buf == expected.as_bytes() {
        Ok(ReadOutcome::Matching)
    } else {
        Ok(ReadOutcome::Mismatch)
    }
}

/// Write a sidecar via a unique same-directory temporary file and an atomic
/// rename, tolerating a concurrent writer that wins the race.
fn write_atomic(dir: &Dir, rel: &Utf8Path, content: &str) -> Result<()> {
    let temp = unique_temp_name(rel);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = match dir.open_with(&temp, &options) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            // Another process won the race for our temporary name; verify the
            // final path and treat matching content as success.
            return match read_verified(dir, rel, content)? {
                ReadOutcome::Matching => Ok(()),
                ReadOutcome::Mismatch => Err(anyhow!(
                    localization::message(keys::RUNNER_IO_DYNDEP_CORRUPT)
                        .with_arg("path", rel.as_str())
                )),
                ReadOutcome::Missing => Err(anyhow!(
                    localization::message(keys::RUNNER_IO_DYNDEP_RACE)
                        .with_arg("path", rel.as_str())
                )),
            };
        }
        Err(err) => {
            return Err(err).with_context(|| {
                localization::message(keys::RUNNER_IO_DYNDEP_WRITE).with_arg("path", rel.as_str())
            });
        }
    };
    file.write_all(content.as_bytes()).with_context(|| {
        localization::message(keys::RUNNER_IO_DYNDEP_WRITE).with_arg("path", rel.as_str())
    })?;
    file.flush().with_context(|| {
        localization::message(keys::RUNNER_IO_DYNDEP_WRITE).with_arg("path", rel.as_str())
    })?;
    file.sync_all().with_context(|| {
        localization::message(keys::RUNNER_IO_DYNDEP_WRITE).with_arg("path", rel.as_str())
    })?;
    // Rename is relative to the same directory; `rename` replaces an existing
    // destination, so if another process already wrote the final file, the
    // atomic replace yields content identical to ours.
    if let Err(err) = dir.rename(&temp, dir, rel) {
        // The final file may have appeared via a concurrent writer; verify it.
        if read_verified(dir, rel, content)? != ReadOutcome::Matching {
            return Err(err).with_context(|| {
                localization::message(keys::RUNNER_IO_DYNDEP_RENAME).with_arg("path", rel.as_str())
            });
        }
        drop(dir.remove_file(&temp));
    }
    Ok(())
}

/// Produce a deterministic, low-collision temporary name beside the final path.
///
/// Uses the sidecar digest plus a fixed suffix; `create_new` guarantees the
/// write never truncates an existing file, and a collision falls back to
/// re-verification of the final content.
fn unique_temp_name(rel: &Utf8Path) -> Utf8PathBuf {
    let name = rel.file_name().unwrap_or("sidecar.dd");
    rel.parent().map_or_else(
        || Utf8PathBuf::from(format!("{name}.tmp")),
        |parent| parent.join(format!("{name}.tmp")),
    )
}

#[cfg(test)]
mod tests {
    //! Unit tests for atomic dyndep sidecar materialization.

    use super::*;
    use crate::ninja_gen::GeneratedDyndep;
    use anyhow::{Result, ensure};
    use camino::Utf8PathBuf;
    use std::fs;

    fn temp_cli(dir: &std::path::Path) -> Cli {
        Cli {
            directory: Some(dir.to_path_buf()),
            ..Cli::default()
        }
    }

    fn sidecar(name: &str, content: &str) -> GeneratedDyndep {
        GeneratedDyndep::fixture(Utf8PathBuf::from(name), content.to_owned())
    }

    #[test]
    fn materializes_nested_sidecar_and_reuses_it() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let cli = temp_cli(temp.path());
        let dyndep = sidecar(".netsuke/dyndep/abc.dd", "ninja_dyndep_version = 1\n");

        materialize_dyndep_files(&cli, &[dyndep])?;
        let final_path = temp.path().join(".netsuke/dyndep/abc.dd");
        ensure_matching(&final_path, "ninja_dyndep_version = 1\n")?;

        // Second run reuses the existing sidecar without error.
        materialize_dyndep_files(
            &cli,
            &[sidecar(
                ".netsuke/dyndep/abc.dd",
                "ninja_dyndep_version = 1\n",
            )],
        )?;
        Ok(())
    }

    #[test]
    fn corrupt_existing_sidecar_is_reported() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let cli = temp_cli(temp.path());
        let final_path = temp.path().join(".netsuke/dyndep/bad.dd");
        fs::create_dir_all(final_path.parent().expect("parent exists"))?;
        fs::write(&final_path, "corrupt")?;

        let result =
            materialize_dyndep_files(&cli, &[sidecar(".netsuke/dyndep/bad.dd", "expected")]);
        ensure!(result.is_err(), "corrupt sidecar must be reported");
        Ok(())
    }

    #[test]
    fn no_temp_files_left_behind() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let cli = temp_cli(temp.path());
        materialize_dyndep_files(&cli, &[sidecar(".netsuke/dyndep/x.dd", "content")])?;
        let dir = temp.path().join(".netsuke/dyndep");
        let leftovers: Vec<_> = fs::read_dir(&dir)?
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| {
                std::path::Path::new(n)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("tmp"))
            })
            .collect();
        ensure!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );
        Ok(())
    }

    fn ensure_matching(path: &std::path::Path, expected: &str) -> Result<()> {
        anyhow::ensure!(
            fs::read_to_string(path)? == expected,
            "sidecar content does not match"
        );
        Ok(())
    }
}
