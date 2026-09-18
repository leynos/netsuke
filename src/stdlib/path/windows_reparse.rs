//! Windows final-component policy: open the reparse point itself and judge the
//! handle that the read will use.
//!
//! Unix reaches this guarantee inside `open` with `O_NOFOLLOW`, so the policy
//! decision and the read share one call. Windows has no flag of that name, but
//! `FILE_FLAG_OPEN_REPARSE_POINT` has the same effect: the handle that comes
//! back refers to the reparse point rather than to whatever it points at. The
//! judgement then reads that handle's own attributes, so nothing between the
//! decision and the read can redirect either one — a concurrent rename in the
//! containing directory changes which entry the path names, but it cannot
//! change what an already-open handle refers to.
//!
//! The policy rejects **every** reparse point, not only the tags `std` reports
//! as symlinks. A junction or volume mount point carries
//! `IO_REPARSE_TAG_MOUNT_POINT`, which is not a name surrogate, so a test
//! phrased as "is this a symlink" would accept exactly the file types this
//! policy exists to refuse. Testing the attribute bit makes the policy express
//! "reject all reparse points" directly.
use camino::Utf8Path;
use cap_std::fs::MetadataExt as _;
use cap_std::fs_utf8::{Metadata, OpenOptions, OpenOptionsExt};
use minijinja::Error;

use super::fs_utils::not_regular_file_error;

/// `FILE_FLAG_OPEN_REPARSE_POINT`: open the reparse point instead of following
/// it. The flag is ignored when the entry is not a reparse point, so it costs
/// an ordinary file nothing.
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

/// `FILE_FLAG_BACKUP_SEMANTICS`: permit a directory to be opened at all.
///
/// Without it an open of a directory fails outright, and the caller would
/// report an open error where Unix reports the regular-file diagnostic. The
/// flag exists to let the shared `is_file` check reject a directory on its
/// attributes, exactly as it does on Unix.
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;

/// `FILE_ATTRIBUTE_REPARSE_POINT`: the entry carries a reparse tag.
///
/// This is the attribute the policy consumes; the tag value itself is
/// deliberately never inspected, so a tag this build has never heard of is
/// refused rather than waved through.
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

/// Apply the Windows half of the open policy to `options`.
///
/// Mirrors `apply_unix_open_flags`: the default policy asks the open itself not
/// to follow a reparse point, and the opt-in policy leaves the open to resolve
/// the link as usual. Directory opens are always permitted, so the shared
/// regular-file check can reject a directory by its attributes.
pub(super) fn apply_open_flags(options: &mut OpenOptions, follow_symlinks: bool) {
    let mut flags = FILE_FLAG_BACKUP_SEMANTICS;
    if !follow_symlinks {
        flags |= FILE_FLAG_OPEN_REPARSE_POINT;
    }
    options.custom_flags(flags);
}

/// Reject an opened handle whose entry is a reparse point.
///
/// `metadata` must come from the handle the caller will read, never from a
/// separate path lookup: that is what makes the refusal race-free.
///
/// # Errors
///
/// Returns the non-regular-file diagnostic when the handle carries the
/// reparse-point attribute.
pub(super) fn reject_reparse_point(metadata: &Metadata, path: &Utf8Path) -> Result<(), Error> {
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(not_regular_file_error(path));
    }
    Ok(())
}
