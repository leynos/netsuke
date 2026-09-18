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
//! The policy rejects **every** reparse point, not only the ones `std` reports
//! as symlinks. `FileType::is_symlink` is a test on the tag *value*: it is true
//! only when the tag is a name surrogate (bit 29 set), which covers symlinks,
//! junctions, and volume mount points, but is false for every other tag — a
//! deduplication or cloud placeholder, for example. An open that follows one of
//! those would hand back the target's handle while a symlink-shaped check saw
//! nothing to refuse. Testing the attribute bit instead asks "is this a reparse
//! point at all", which is the policy the callers actually want and needs no
//! knowledge of which tags a future Windows release may mint.
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

/// The `dwFlagsAndAttributes` bits the policy passes to the open.
///
/// `FILE_FLAG_BACKUP_SEMANTICS` is unconditional, so a directory can be opened
/// and then rejected by the shared regular-file check rather than by the open
/// failing. `FILE_FLAG_OPEN_REPARSE_POINT` is added only by the default policy,
/// which must not traverse a reparse point.
pub(super) const fn open_flags(follow_symlinks: bool) -> u32 {
    let mut flags = FILE_FLAG_BACKUP_SEMANTICS;
    if !follow_symlinks {
        flags |= FILE_FLAG_OPEN_REPARSE_POINT;
    }
    flags
}

/// Apply the Windows half of the open policy to `options`.
///
/// Mirrors `apply_unix_open_flags`: the default policy asks the open itself not
/// to follow a reparse point, and the opt-in policy leaves the open to resolve
/// the link as usual.
pub(super) fn apply_open_flags(options: &mut OpenOptions, follow_symlinks: bool) {
    options.custom_flags(open_flags(follow_symlinks));
}

/// Whether `file_attributes` describes a reparse point the policy refuses.
///
/// The test is on the attribute bit alone, and deliberately never on the tag
/// value. A tag-value test would have to enumerate acceptable tags, and a
/// name-surrogate test — the shape `FileType::is_symlink` uses — misses every
/// tag that is not a name surrogate, such as a deduplication or cloud
/// placeholder.
pub(super) const fn is_prohibited_reparse_point(file_attributes: u32) -> bool {
    file_attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
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
    if is_prohibited_reparse_point(metadata.file_attributes()) {
        return Err(not_regular_file_error(path));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default policy must ask the open not to traverse a reparse point,
    /// and both policies must permit a directory open so the shared
    /// regular-file check can report the documented rejection.
    #[test]
    fn default_policy_opens_without_traversing_and_permits_directories() {
        let default = open_flags(false);
        assert_eq!(
            default & FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_FLAG_OPEN_REPARSE_POINT,
            "the default policy must not traverse a reparse point"
        );
        assert_eq!(
            default & FILE_FLAG_BACKUP_SEMANTICS,
            FILE_FLAG_BACKUP_SEMANTICS,
            "the default policy must still permit opening a directory"
        );

        let following = open_flags(true);
        assert_eq!(
            following & FILE_FLAG_OPEN_REPARSE_POINT,
            0,
            "the opt-in policy must let the open resolve the link"
        );
        assert_eq!(
            following & FILE_FLAG_BACKUP_SEMANTICS,
            FILE_FLAG_BACKUP_SEMANTICS,
            "the opt-in policy must still permit opening a directory"
        );
    }

    /// The three constants are Windows ABI values, so a transcription slip
    /// would compile and misbehave. Pin them against the documented values.
    #[test]
    fn constants_match_the_documented_abi_values() {
        assert_eq!(FILE_FLAG_OPEN_REPARSE_POINT, 0x0020_0000);
        assert_eq!(FILE_FLAG_BACKUP_SEMANTICS, 0x0200_0000);
        assert_eq!(FILE_ATTRIBUTE_REPARSE_POINT, 0x0000_0400);
    }

    /// The policy refuses every reparse tag, including the ones a
    /// name-surrogate test would miss.
    ///
    /// `FileType::is_symlink` is true only when the tag is a name surrogate
    /// (bit 29 set) — symlinks, junctions, and volume mount points — and
    /// false for every other tag, for which `std` also reports
    /// `is_file() == true`. So a tag-value test would both miss the
    /// non-surrogate tags below and gain nothing for the surrogate ones: the
    /// shared regular-file check already refuses a junction. The attribute
    /// bit is the one test that covers both.
    #[test]
    fn policy_refuses_surrogate_and_non_surrogate_tags_alike() {
        /// `IO_REPARSE_TAG_SYMLINK`
        const SYMLINK: u32 = 0xA000_000C;
        /// `IO_REPARSE_TAG_MOUNT_POINT`
        const MOUNT_POINT: u32 = 0xA000_0003;
        /// `IO_REPARSE_TAG_NFS`
        const NFS: u32 = 0x8000_0014;
        /// `IO_REPARSE_TAG_DEDUP`
        const DEDUP: u32 = 0x8000_0013;
        /// `IO_REPARSE_TAG_CLOUD`
        const CLOUD: u32 = 0x9000_001A;

        /// Bit 29: the name-surrogate flag.
        const NAME_SURROGATE: u32 = 0x2000_0000;
        /// Every tag stamped onto an entry carries the attribute bit.
        const ON_DISK: u32 = FILE_ATTRIBUTE_REPARSE_POINT;
        /// `FILE_ATTRIBUTE_ARCHIVE`: what an ordinary file carries instead.
        const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x0000_0020;

        for (tag, is_surrogate) in [
            (SYMLINK, true),
            (MOUNT_POINT, true),
            (NFS, false),
            (DEDUP, false),
            (CLOUD, false),
        ] {
            assert_eq!(
                tag & NAME_SURROGATE != 0,
                is_surrogate,
                "test fixture for tag {tag:#010x} misstates the surrogate flag"
            );
            assert!(
                is_prohibited_reparse_point(tag | ON_DISK),
                "tag {tag:#010x} (name surrogate: {is_surrogate}) must be refused"
            );
        }
        assert!(
            !is_prohibited_reparse_point(FILE_ATTRIBUTE_ARCHIVE),
            "an ordinary file must not be refused"
        );
    }
}
