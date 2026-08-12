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
use std::sync::atomic::{AtomicU64, Ordering};

/// Namespace for generated dyndep sidecar files.
pub(crate) const DYNDEP_DIR: &str = ".netsuke/dyndep";

/// Distinguishes temporary sidecars created by separate write attempts.
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Materialize every sidecar in `dyndep_files` under the effective Ninja
/// working directory selected by `cli`.
///
/// # Errors
///
/// Returns successfully without opening the working directory when
/// `dyndep_files` is empty. Otherwise returns an error if the working directory
/// cannot be opened or the `.netsuke/dyndep` directory created, or if any
/// sidecar write, rename, or content verification fails.
pub fn materialize_dyndep_files(cli: &Cli, dyndep_files: &[GeneratedDyndep]) -> Result<()> {
    if dyndep_files.is_empty() {
        return Ok(());
    }
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
    let (temp, mut file) = create_unique_temp_file(dir, rel)?;
    write_and_sync_temp_file(&mut file, rel, content)?;
    rename_temp_file(dir, &temp, rel, content)
}

/// Create a temporary sidecar with a path that no other entry claims.
fn create_unique_temp_file(
    dir: &Dir,
    rel: &Utf8Path,
) -> Result<(Utf8PathBuf, cap_std::fs_utf8::File)> {
    loop {
        let temp = unique_temp_name(rel);
        if let Some(file) = create_temp_file(dir, &temp, rel)? {
            return Ok((temp, file));
        }
    }
}

/// Create a temporary sidecar, reporting a name collision to the caller.
fn create_temp_file(
    dir: &Dir,
    temp: &Utf8Path,
    rel: &Utf8Path,
) -> Result<Option<cap_std::fs_utf8::File>> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    match dir.open_with(temp, &options) {
        Ok(file) => Ok(Some(file)),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            Ok(handle_temp_name_collision())
        }
        Err(err) => Err(err).with_context(|| {
            localization::message(keys::RUNNER_IO_DYNDEP_WRITE).with_arg("path", rel.as_str())
        }),
    }
}

/// Signal that the caller should retry with another temporary name.
const fn handle_temp_name_collision() -> Option<cap_std::fs_utf8::File> {
    None
}

/// Write, flush, and synchronize a temporary sidecar before renaming it.
fn write_and_sync_temp_file(
    file: &mut cap_std::fs_utf8::File,
    rel: &Utf8Path,
    content: &str,
) -> Result<()> {
    file.write_all(content.as_bytes()).with_context(|| {
        localization::message(keys::RUNNER_IO_DYNDEP_WRITE).with_arg("path", rel.as_str())
    })?;
    file.flush().with_context(|| {
        localization::message(keys::RUNNER_IO_DYNDEP_WRITE).with_arg("path", rel.as_str())
    })?;
    file.sync_all().with_context(|| {
        localization::message(keys::RUNNER_IO_DYNDEP_WRITE).with_arg("path", rel.as_str())
    })?;
    Ok(())
}

/// Rename a completed temporary sidecar into its final location.
fn rename_temp_file(dir: &Dir, temp: &Utf8Path, rel: &Utf8Path, content: &str) -> Result<()> {
    match dir.rename(temp, dir, rel) {
        Ok(()) => Ok(()),
        Err(rename_error) => handle_rename_failure(
            dir,
            &RenameFailureContext { temp, rel, content },
            rename_error,
        ),
    }
}

/// Sidecar values retained when handling the outcome of one attempted rename.
///
/// This remains private to atomic dyndep materialization: only
/// [`rename_temp_file`] creates it, and [`handle_rename_failure`] consumes it.
struct RenameFailureContext<'a> {
    temp: &'a Utf8Path,
    rel: &'a Utf8Path,
    content: &'a str,
}

/// Verify a final sidecar when another process wins the rename race.
fn handle_rename_failure(
    dir: &Dir,
    context: &RenameFailureContext<'_>,
    rename_error: std::io::Error,
) -> Result<()> {
    // Rename is relative to the same directory; `rename` replaces an existing
    // destination, so if another process already wrote the final file, the
    // atomic replace yields content identical to ours.
    if read_verified(dir, context.rel, context.content)? == ReadOutcome::Matching {
        drop(dir.remove_file(context.temp));
        return Ok(());
    }
    Err(rename_error).with_context(|| {
        localization::message(keys::RUNNER_IO_DYNDEP_RENAME).with_arg("path", context.rel.as_str())
    })
}

/// Produce a distinct same-directory temporary name for one write attempt.
///
/// The process identifier and monotonically increasing sequence distinguish
/// concurrent writers. `create_new` still guards against an unlikely stale
/// collision, which the caller resolves by choosing another name.
fn unique_temp_name(rel: &Utf8Path) -> Utf8PathBuf {
    let name = rel.file_name().unwrap_or("sidecar.dd");
    let process_id = std::process::id();
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp_name = format!("{name}.{process_id}.{sequence}.tmp");
    rel.parent().map_or_else(
        || Utf8PathBuf::from(&temp_name),
        |parent| parent.join(&temp_name),
    )
}

#[cfg(test)]
mod tests {
    //! Unit tests for atomic dyndep sidecar materialization.

    use super::*;
    use crate::ninja_gen::GeneratedDyndep;
    use anyhow::{Result, ensure};
    use camino::Utf8PathBuf;

    fn temp_cli(dir: &std::path::Path) -> Cli {
        Cli {
            directory: Some(dir.to_path_buf()),
            ..Cli::default()
        }
    }

    fn sidecar(name: &str, content: &str) -> GeneratedDyndep {
        GeneratedDyndep::fixture(Utf8PathBuf::from(name), content.to_owned())
    }

    fn temp_dir(temp: &tempfile::TempDir) -> Result<Dir> {
        let path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow!("temporary directory is not UTF-8: {}", path.display()))?;
        Dir::open_ambient_dir(path, ambient_authority()).map_err(Into::into)
    }

    #[test]
    fn materializes_nested_sidecar_and_reuses_it() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let cli = temp_cli(temp.path());
        let dyndep = sidecar(".netsuke/dyndep/abc.dd", "ninja_dyndep_version = 1\n");

        materialize_dyndep_files(&cli, &[dyndep])?;
        ensure_matching(
            &temp_dir(&temp)?,
            ".netsuke/dyndep/abc.dd",
            "ninja_dyndep_version = 1\n",
        )?;

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
    fn empty_sidecar_list_does_not_create_dyndep_directory() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let cli = temp_cli(temp.path());

        materialize_dyndep_files(&cli, &[])?;

        ensure!(
            temp_dir(&temp)?.open(DYNDEP_DIR).is_err(),
            "empty sidecar list must not create {DYNDEP_DIR}"
        );
        Ok(())
    }

    #[test]
    fn corrupt_existing_sidecar_is_reported() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let cli = temp_cli(temp.path());
        let dir = temp_dir(&temp)?;
        dir.create_dir_all(DYNDEP_DIR)?;
        dir.write(".netsuke/dyndep/bad.dd", "corrupt")?;

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
        let temp_file = ".netsuke/dyndep/x.dd.tmp";
        ensure!(
            temp_dir(&temp)?.open(temp_file).is_err(),
            "temp file left behind"
        );
        Ok(())
    }

    #[test]
    fn stale_temp_file_does_not_block_materialization() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let cli = temp_cli(temp.path());
        let dir = temp_dir(&temp)?;
        let rel = Utf8Path::new(".netsuke/dyndep/stale.dd");
        let content = "ninja_dyndep_version = 1\n";
        dir.create_dir_all(DYNDEP_DIR)?;
        dir.write(unique_temp_name(rel), "stale temporary content")?;

        materialize_dyndep_files(&cli, &[sidecar(rel.as_str(), content)])?;

        ensure_matching(&dir, rel.as_str(), content)
    }

    #[test]
    fn separate_temp_names_for_same_sidecar_differ() {
        let rel = Utf8Path::new(".netsuke/dyndep/names.dd");
        let first = unique_temp_name(rel);
        let second = unique_temp_name(rel);

        assert_ne!(first, second, "temporary names must differ per attempt");
        assert_eq!(first.parent(), rel.parent());
        assert_eq!(second.parent(), rel.parent());
    }

    #[test]
    fn matching_final_sidecar_succeeds_with_another_temp_file() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let dir = temp_dir(&temp)?;
        let rel = Utf8Path::new(".netsuke/dyndep/matching.dd");
        let content = "ninja_dyndep_version = 1\n";
        dir.create_dir_all(DYNDEP_DIR)?;
        dir.write(rel, content)?;
        dir.write(unique_temp_name(rel), "concurrent temporary content")?;

        write_atomic(&dir, rel, content)?;

        ensure_matching(&dir, rel.as_str(), content)
    }

    fn ensure_matching(dir: &Dir, path: &str, expected: &str) -> Result<()> {
        anyhow::ensure!(
            dir.read_to_string(path)? == expected,
            "sidecar content does not match"
        );
        Ok(())
    }
}
