//! Bounded reads of opened files for the stdlib filters.
//!
//! A read streams through a fixed-size buffer, charging the byte budget as
//! bytes arrive, so no buffer grows with the length of a single line. Text is
//! validated incrementally: a file that is not valid UTF-8 is rejected rather
//! than counted as opaque bytes, and the byte-budget diagnostic is raised here.
use std::io::Read;

use camino::Utf8Path;
use cap_std::fs_utf8::File;
use minijinja::Error;

use super::fs_utils::{FileReadLimits, open_file_checked};
use crate::localization::{self, keys};
use crate::stdlib::io_helpers::io_to_error;

/// Read a bounded chunk of `file`, rejecting reads that exceed `max_bytes`.
///
/// Returns the bytes read so far, or `None` when the source has been
/// exhausted. Returns an error once the running total passes `max_bytes`.
pub(crate) fn read_bounded_chunk<'a>(
    state: &mut BoundedRead,
    file: &mut File,
    buffer: &'a mut [u8],
    path: &Utf8Path,
) -> Result<Option<&'a [u8]>, Error> {
    let read = file.read(buffer).map_err(|err| {
        io_to_error(
            path,
            &localization::message(keys::STDLIB_PATH_ACTION_READ),
            err,
        )
    })?;
    if read == 0 {
        return Ok(None);
    }
    state.total = state
        .total
        .saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
    if state.total > state.max_bytes {
        return Err(file_too_large_error(path, state.max_bytes));
    }
    // `Read::read` cannot report more than the buffer holds, so index the
    // slice defensively and treat an over-report as an empty chunk.
    Ok(Some(buffer.get(..read).unwrap_or(&[])))
}

/// Running byte total for a bounded read against the configured ceiling.
pub(crate) struct BoundedRead {
    /// Bytes consumed so far by this read.
    total: u64,
    /// The budget this read may not exceed.
    max_bytes: u64,
}

impl BoundedRead {
    /// Start a bounded read with a fresh running total under `max_bytes`.
    pub(crate) const fn new(max_bytes: u64) -> Self {
        Self {
            total: 0,
            max_bytes,
        }
    }
}

/// Build the localized byte-budget diagnostic for `path` and `limit`.
pub(crate) fn file_too_large_error(path: &Utf8Path, limit: u64) -> Error {
    Error::new(
        minijinja::ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_PATH_FILE_TOO_LARGE)
            .with_arg("path", path.as_str())
            .with_arg("limit", limit)
            .to_string(),
    )
}

/// Build the localized invalid-UTF-8 diagnostic for `path`.
fn invalid_utf8_error(path: &Utf8Path) -> Error {
    Error::new(
        minijinja::ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_PATH_IO_INVALID_DATA)
            .with_arg("path", path.as_str())
            .to_string(),
    )
}

/// Read the file at `path` as UTF-8 text.
///
/// # Errors
///
/// Returns a template error when the parent directory cannot be opened, the
/// file cannot be read, its contents are not valid UTF-8, or the read exceeds
/// the configured byte budget.
pub(crate) fn read_utf8(path: &Utf8Path, limits: &FileReadLimits) -> Result<String, Error> {
    let mut file = open_file_checked(path, limits)?;
    let mut state = BoundedRead::new(limits.max_bytes);
    let mut buffer = [0_u8; 8192];
    let mut bytes = Vec::new();
    while let Some(chunk) = read_bounded_chunk(&mut state, &mut file, &mut buffer, path)? {
        bytes.extend_from_slice(chunk);
    }
    String::from_utf8(bytes).map_err(|_| invalid_utf8_error(path))
}

/// Incremental UTF-8 validator that carries a split sequence between chunks.
///
/// The line counter validates each chunk as it arrives instead of buffering
/// the whole file to prove it is text. Only a truncated trailing sequence is
/// retained: at most the three bytes of a multi-byte sequence whose remaining
/// bytes lie in the next chunk.
#[derive(Default)]
struct Utf8Validator {
    /// Bytes of a trailing multi-byte sequence awaiting its continuation.
    pending: Vec<u8>,
}

impl Utf8Validator {
    /// Validate `chunk`, retaining any truncated multi-byte sequence at its end.
    ///
    /// # Errors
    ///
    /// Returns a template error when `chunk` contains invalid UTF-8, or when it
    /// completes a sequence the previous chunk left truncated with an invalid
    /// byte.
    fn push(&mut self, chunk: &[u8], path: &Utf8Path) -> Result<(), Error> {
        self.pending.extend_from_slice(chunk);
        // Take an owned error so the borrow of `pending` ends here; the match
        // below needs `pending` mutably to retain the truncated tail.
        let error = std::str::from_utf8(&self.pending).err();
        match error {
            None => {
                self.pending.clear();
                Ok(())
            }
            Some(err) if err.error_len().is_some() => Err(invalid_utf8_error(path)),
            Some(err) => {
                // `error_len` is `None` only for a sequence the chunk ended in
                // the middle of, so the tail is at most three bytes.
                let truncated = self.pending.split_off(err.valid_up_to());
                self.pending = truncated;
                Ok(())
            }
        }
    }

    /// Reject a file that ends in the middle of a multi-byte sequence.
    ///
    /// # Errors
    ///
    /// Returns a template error when a truncated sequence is still pending.
    fn finish(&self, path: &Utf8Path) -> Result<(), Error> {
        if self.pending.is_empty() {
            Ok(())
        } else {
            Err(invalid_utf8_error(path))
        }
    }
}

/// Count the lines in the file at `path`.
///
/// The read streams through fixed-size chunks, so the byte budget is enforced
/// as bytes arrive and no buffer grows with the length of a single line. The
/// file must be UTF-8 text, matching `contents`: invalid UTF-8 is rejected
/// rather than counted as opaque bytes.
///
/// # Errors
///
/// Returns a template error when the file cannot be opened or read, when its
/// contents are not valid UTF-8, or when the read exceeds the configured byte
/// budget.
pub(crate) fn linecount(path: &Utf8Path, limits: &FileReadLimits) -> Result<usize, Error> {
    let mut file = open_file_checked(path, limits)?;
    let mut state = BoundedRead::new(limits.max_bytes);
    let mut buffer = [0_u8; 8192];
    let mut utf8 = Utf8Validator::default();
    let mut lines: usize = 0;
    let mut read_any = false;
    let mut ends_with_newline = false;
    while let Some(chunk) = read_bounded_chunk(&mut state, &mut file, &mut buffer, path)? {
        utf8.push(chunk, path)?;
        for byte in chunk {
            if *byte == b'\n' {
                lines += 1;
            }
        }
        read_any = true;
        ends_with_newline = chunk.last() == Some(&b'\n');
    }
    utf8.finish(path)?;
    // A trailing fragment after the last terminator is a line of its own; an
    // empty file has no lines at all.
    if read_any && !ends_with_newline {
        lines += 1;
    }
    Ok(lines)
}

#[cfg(test)]
#[path = "bounded_read_tests.rs"]
mod tests;
