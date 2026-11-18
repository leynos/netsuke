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
//! All writes go through the verified dyndep directory handle, using a
//! same-directory temporary file and an atomic rename so concurrent Netsuke
//! processes cannot observe partial content. Every path below that handle is a
//! bare entry name, so publication cannot resolve a component outside the
//! directory the handle verified.

use super::super::DYNDEP_DIR;
use super::directory::{DyndepDirectory, entry_display_path};
use super::{retention::DyndepPublicationLease, telemetry};
use crate::localization::{self, keys};
use crate::ninja_gen::GeneratedDyndep;
use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::{Dir, OpenOptions};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum number of colliding temporary names tolerated per write.
const MAX_TEMP_FILE_ATTEMPTS: usize = 16;
/// Maximum existing dyndep sidecar size accepted during verification.
///
/// Generated sidecars are small Ninja fragments. A 16 MiB ceiling leaves
/// ample room for large manifests while preventing an untrusted existing file
/// from driving an unbounded allocation during publication.
const MAX_VERIFIED_DYNDEP_SIZE: u64 = 16 * 1024 * 1024;

/// Materialize every sidecar in `dyndep_files` through `workspace`.
///
/// # Errors
///
/// Returns successfully without creating generated state when `dyndep_files`
/// is empty. Otherwise returns an error if the `.netsuke/dyndep` directory
/// cannot be established, is a symbolic link, junction, or non-directory, or if
/// any sidecar write, rename, or content verification fails.
pub(crate) fn materialize_dyndep_files(
    workspace: &Dir,
    dyndep_files: &[GeneratedDyndep],
) -> Result<DyndepPublicationLease> {
    telemetry::instrument_materialization(dyndep_files.len(), || {
        materialize_dyndep_files_inner(workspace, dyndep_files)
    })
}

/// Apply the sidecar publication command through the supplied capability.
fn materialize_dyndep_files_inner(
    workspace: &Dir,
    dyndep_files: &[GeneratedDyndep],
) -> Result<DyndepPublicationLease> {
    if dyndep_files.is_empty() {
        return Ok(DyndepPublicationLease::empty());
    }
    let dyndep = DyndepDirectory::open_or_create(workspace)?;
    let lease = DyndepPublicationLease::acquire(&dyndep)?;
    for sidecar in dyndep_files {
        materialize_one(dyndep.dir(), sidecar)?;
    }
    Ok(lease)
}

/// Materialize one sidecar idempotently and atomically.
fn materialize_one(dyndep: &Dir, sidecar: &GeneratedDyndep) -> Result<()> {
    let name = sidecar_name(sidecar)?;
    telemetry::instrument_sidecar_materialization(|| {
        materialize_one_inner(dyndep, name, sidecar.content())
    })
}

/// Return the sidecar's bare entry name within the verified directory.
///
/// # Errors
///
/// Returns an error when the sidecar path is not a direct child of
/// `.netsuke/dyndep`, which would place it outside the verified handle.
fn sidecar_name(sidecar: &GeneratedDyndep) -> Result<&Utf8Path> {
    let path = sidecar.relative_path();
    let name = path.file_name().unwrap_or_default();
    if path.parent() == Some(Utf8Path::new(DYNDEP_DIR)) && !name.is_empty() {
        Ok(Utf8Path::new(name))
    } else {
        Err(anyhow!(
            localization::message(keys::RUNNER_IO_DYNDEP_WRITE).with_arg("path", path.as_str())
        ))
    }
}

/// Publish or verify one sidecar without attaching observability policy.
fn materialize_one_inner(dyndep: &Dir, name: &Utf8Path, content: &str) -> Result<()> {
    match read_verified(dyndep, name, content)? {
        ReadOutcome::Matching => {
            tracing::debug!(operation = "reuse", "reusing existing dyndep sidecar",);
            Ok(())
        }
        ReadOutcome::Mismatch => Err(anyhow!(
            localization::message(keys::RUNNER_IO_DYNDEP_CORRUPT)
                .with_arg("path", entry_display_path(name.as_str()))
        )),
        ReadOutcome::Missing => write_atomic(dyndep, name, content),
    }
}

/// Result of comparing an existing sidecar with its expected content.
#[derive(PartialEq)]
enum ReadOutcome {
    /// Existing sidecar bytes match the expected content.
    Matching,
    /// Existing sidecar bytes differ or changed while being read.
    Mismatch,
    /// No sidecar exists yet.
    Missing,
}

/// Read an existing sidecar and compare it with the expected content.
fn read_verified(dyndep: &Dir, name: &Utf8Path, expected: &str) -> Result<ReadOutcome> {
    let Some(mut file) = open_existing_sidecar(dyndep, name)? else {
        return Ok(ReadOutcome::Missing);
    };
    let size = verified_sidecar_size(&file, name)?;
    let (content, grew_while_reading) = read_sidecar_content(&mut file, size, name)?;
    Ok(content_outcome(&content, expected, grew_while_reading))
}

/// Open an existing sidecar for reading, mapping a missing file to `None`.
///
/// # Errors
///
/// Returns an error when the sidecar cannot be read for any reason other than
/// its absence.
fn open_existing_sidecar(dyndep: &Dir, name: &Utf8Path) -> Result<Option<cap_std::fs_utf8::File>> {
    let mut options = OpenOptions::new();
    options.read(true);
    match dyndep.open_with(name, &options) {
        Ok(file) => Ok(Some(file)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| {
            localization::message(keys::RUNNER_IO_DYNDEP_READ)
                .with_arg("path", entry_display_path(name.as_str()))
        }),
    }
}

/// Return the existing sidecar's byte length, rejecting oversize files.
///
/// # Errors
///
/// Returns an error when the file metadata cannot be read or the size exceeds
/// the accepted verification ceiling.
fn verified_sidecar_size(file: &cap_std::fs_utf8::File, name: &Utf8Path) -> Result<u64> {
    let display = entry_display_path(name.as_str());
    let size = file
        .metadata()
        .with_context(|| {
            localization::message(keys::RUNNER_IO_DYNDEP_READ).with_arg("path", display.as_str())
        })?
        .len();
    if size > MAX_VERIFIED_DYNDEP_SIZE {
        return Err(anyhow!(
            localization::message(keys::RUNNER_IO_DYNDEP_TOO_LARGE)
                .with_arg("path", display.as_str())
                .with_arg("limit", MAX_VERIFIED_DYNDEP_SIZE)
        ));
    }
    Ok(size)
}

/// Read exactly the verified byte count, flagging growth beyond it.
///
/// The returned flag tells the caller whether the file grew past the verified
/// size while being read, which would make a byte match untrustworthy.
///
/// # Errors
///
/// Returns an error when the sidecar cannot be read.
fn read_sidecar_content(
    file: &mut cap_std::fs_utf8::File,
    size: u64,
    name: &Utf8Path,
) -> Result<(Vec<u8>, bool)> {
    let display = entry_display_path(name.as_str());
    let read_error =
        || localization::message(keys::RUNNER_IO_DYNDEP_READ).with_arg("path", display.as_str());
    let verified_size = usize::try_from(size).with_context(read_error)?;
    let mut buf = Vec::with_capacity(verified_size);
    Read::by_ref(file)
        .take(size)
        .read_to_end(&mut buf)
        .with_context(read_error)?;
    let mut growth_probe = [0_u8; 1];
    let grew_while_reading = file.read(&mut growth_probe).with_context(read_error)? != 0;
    Ok((buf, grew_while_reading))
}

/// Classify read content as matching only when it is exact and stable.
fn content_outcome(content: &[u8], expected: &str, grew_while_reading: bool) -> ReadOutcome {
    if !grew_while_reading && content == expected.as_bytes() {
        ReadOutcome::Matching
    } else {
        ReadOutcome::Mismatch
    }
}

/// Write a sidecar via a unique same-directory temporary file and an atomic
/// rename, tolerating a concurrent writer that wins the race.
fn write_atomic(dyndep: &Dir, name: &Utf8Path, content: &str) -> Result<()> {
    let (temp, mut file) = create_unique_temp_file(dyndep, name)?;
    let write_result = write_and_sync_temp_file(&mut file, name, content);
    drop(file);
    let result = write_result.and_then(|()| rename_temp_file(dyndep, &temp, name, content));
    if result.is_err() {
        drop(dyndep.remove_file(&temp));
    }
    result
}

/// Create a temporary sidecar with a path that no other entry claims.
fn create_unique_temp_file(
    dyndep: &Dir,
    name: &Utf8Path,
) -> Result<(Utf8PathBuf, cap_std::fs_utf8::File)> {
    let mut names = TempNameSource::for_operation();
    create_unique_temp_file_with_source(dyndep, name, &mut names)
}

/// Create a temporary sidecar from an operation-scoped candidate source.
fn create_unique_temp_file_with_source(
    dyndep: &Dir,
    name: &Utf8Path,
    names: &mut TempNameSource,
) -> Result<(Utf8PathBuf, cap_std::fs_utf8::File)> {
    for _ in 0..MAX_TEMP_FILE_ATTEMPTS {
        let temp = names.next_name(name);
        if let Some(file) = create_temp_file(dyndep, &temp, name)? {
            return Ok((temp, file));
        }
        telemetry::record_temp_file_retry("retry");
    }
    telemetry::record_temp_file_retry("exhausted");
    Err(anyhow!(
        localization::message(keys::RUNNER_IO_DYNDEP_TEMP_COLLISIONS)
            .with_arg("path", entry_display_path(name.as_str()))
    ))
}

/// Create a temporary sidecar, reporting a name collision to the caller.
fn create_temp_file(
    dyndep: &Dir,
    temp: &Utf8Path,
    name: &Utf8Path,
) -> Result<Option<cap_std::fs_utf8::File>> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    match dyndep.open_with(temp, &options) {
        Ok(file) => Ok(Some(file)),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
        Err(err) => Err(err).with_context(|| {
            localization::message(keys::RUNNER_IO_DYNDEP_WRITE)
                .with_arg("path", entry_display_path(name.as_str()))
        }),
    }
}

/// Write, flush, and synchronize a temporary sidecar before renaming it.
fn write_and_sync_temp_file(
    file: &mut cap_std::fs_utf8::File,
    name: &Utf8Path,
    content: &str,
) -> Result<()> {
    let display = entry_display_path(name.as_str());
    let write_error =
        || localization::message(keys::RUNNER_IO_DYNDEP_WRITE).with_arg("path", display.as_str());
    file.write_all(content.as_bytes())
        .with_context(write_error)?;
    file.flush().with_context(write_error)?;
    file.sync_all().with_context(write_error)?;
    Ok(())
}

/// Rename a completed temporary sidecar into its final location.
fn rename_temp_file(dyndep: &Dir, temp: &Utf8Path, name: &Utf8Path, content: &str) -> Result<()> {
    match dyndep.rename(temp, dyndep, name) {
        Ok(()) => Ok(()),
        Err(rename_error) => handle_rename_failure(
            dyndep,
            &RenameAttempt {
                temp,
                name,
                content,
            },
            rename_error,
        ),
    }
}

/// Paths and expected content for one failed same-directory rename.
struct RenameAttempt<'a> {
    /// Same-directory temporary path of the finished sidecar.
    temp: &'a Utf8Path,
    /// Final sidecar entry name.
    name: &'a Utf8Path,
    /// Expected content, used to verify a concurrent writer's result.
    content: &'a str,
}

/// Verify a final sidecar when another process wins the rename race.
fn handle_rename_failure(
    dyndep: &Dir,
    attempt: &RenameAttempt<'_>,
    rename_error: std::io::Error,
) -> Result<()> {
    // Rename is relative to the same directory; `rename` replaces an existing
    // destination, so if another process already wrote the final file, the
    // atomic replace yields content identical to ours.
    if read_verified(dyndep, attempt.name, attempt.content)? == ReadOutcome::Matching {
        drop(dyndep.remove_file(attempt.temp));
        return Ok(());
    }
    Err(rename_error).with_context(|| {
        localization::message(keys::RUNNER_IO_DYNDEP_RENAME)
            .with_arg("path", entry_display_path(attempt.name.as_str()))
    })
}

/// Operation-scoped source of same-directory temporary path candidates.
///
/// This private source is used only by dyndep publication. Its nonce is a
/// best-effort collision-reduction suffix, while its local sequence supports
/// bounded retries without process-global mutable state. `create_new` remains
/// the authority that decides whether each candidate is available.
struct TempNameSource {
    /// Best-effort suffix that reduces candidate-name collisions.
    nonce: String,
    /// Local candidate counter for bounded collision retries.
    sequence: u64,
}

impl TempNameSource {
    /// Build an operation-scoped source with a best-effort nonce.
    fn for_operation() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut thread_hasher = DefaultHasher::new();
        std::thread::current().id().hash(&mut thread_hasher);
        let nonce = format!(
            "{}.{}.{}",
            std::process::id(),
            timestamp,
            thread_hasher.finish()
        );
        Self::new(nonce)
    }

    /// Construct a source from a caller-supplied nonce.
    const fn new(nonce: String) -> Self {
        Self { nonce, sequence: 0 }
    }

    /// Return the next candidate temporary path beside `name`.
    fn next_name(&mut self, name: &Utf8Path) -> Utf8PathBuf {
        let file_name = name.file_name().unwrap_or("sidecar.dd");
        let sequence = self.sequence;
        self.sequence += 1;
        let temp_name = format!("{file_name}.{}.{sequence}.tmp", self.nonce);
        name.parent().map_or_else(
            || Utf8PathBuf::from(&temp_name),
            |parent| parent.join(&temp_name),
        )
    }
}

#[cfg(test)]
mod tests;
