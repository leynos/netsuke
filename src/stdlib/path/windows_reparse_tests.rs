//! Unit tests for the Windows reparse-point open policy.
//!
//! The handle tests need a real junction, so they skip — loudly — when the host
//! has no `cmd.exe` to reach `mklink` through. A junction needs no privilege, so
//! that is the only condition under which the fixture is unavailable: every
//! other setup failure propagates as an error rather than quietly turning the
//! test green.

use super::*;
#[cfg(windows)]
use anyhow::{Context, Result, ensure};
#[cfg(windows)]
use camino::Utf8PathBuf;
#[cfg(windows)]
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, File},
};
#[cfg(windows)]
use tempfile::tempdir;

/// Name of the junction the handle tests create.
#[cfg(windows)]
const LINK: &str = "junc";
/// Name of the directory that junction points at.
#[cfg(windows)]
const TARGET: &str = "junction_target";

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

/// Everything the junction test needs, kept alive for its whole body.
///
/// The temporary directory is held here so the tree outlives the handles taken
/// from it; dropping the fixture removes the junction with it.
#[cfg(windows)]
struct JunctionFixture {
    /// Owns the tree; dropping it removes the junction.
    _temp: tempfile::TempDir,
    /// Capability over the fixture workspace.
    dir: Dir,
    /// The junction opened with the default policy. It is a directory-shaped
    /// reparse point, and the capability resolver cannot traverse it, so an
    /// ambient open is the only way to reach the reparse point itself.
    opened: File,
    /// The link name inside the workspace.
    link: Utf8PathBuf,
    /// The directory the link points at.
    target: Utf8PathBuf,
}

/// Open `path` through the ambient authority with the default policy.
///
/// `path` must be absolute: the ambient open resolves it directly, without the
/// capability resolver that refuses an absolute link target.
///
/// # Errors
///
/// Returns the open error when the entry cannot be opened, or when the
/// requested access mode and creation disposition are inconsistent.
#[cfg(windows)]
fn open_link_ambient(path: &Utf8Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    apply_open_flags(&mut options, false);
    File::open_ambient_with(path, &options, ambient_authority())
        .with_context(|| format!("open the junction fixture at {path}"))
}

/// Create the workspace the handle tests share.
///
/// `Ok(None)` means this host has no `cmd.exe` to reach `mklink` through, so the
/// fixture cannot be built; the caller reports that as a skip rather than a
/// failure, exactly as `fallible::junction_fixture` does in the integration
/// suite. A junction needs no privilege, so a missing `cmd.exe` is the only
/// unavailability that is a skip — every other failure is a real fault, and
/// propagates as an error.
///
/// The helper returns `Result` rather than calling `expect`, and hands back a
/// `File` rather than a platform handle, so it needs no unsafe code.
///
/// # Errors
///
/// Returns the setup error when `cmd` is present but the junction was not
/// created, quoting `mklink`'s own diagnostics.
#[cfg(windows)]
fn junction_fixture() -> Result<Option<JunctionFixture>> {
    use std::os::windows::process::CommandExt as _;
    use std::process::Command;

    let temp = tempdir().context("create the junction fixture workspace")?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|_| anyhow::anyhow!("fixture workspace path is valid UTF-8"))?;
    let dir = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open the fixture workspace as a capability")?;
    dir.create_dir(TARGET)
        .context("create the junction fixture target directory")?;
    let link = root.join(LINK);
    let target = root.join(TARGET);
    ensure!(
        !link.as_str().contains('"') && !target.as_str().contains('"'),
        "workspace path contains a quote and cannot be passed to cmd: \
         link {link}, target {target}"
    );

    // `mklink` is a `cmd` built-in, so it is reachable only through `cmd /C`.
    // `raw_arg` passes the line verbatim because `cmd` parses it itself, rather
    // than by the C runtime's argument-quoting rules.
    let output = match Command::new("cmd")
        .arg("/C")
        .raw_arg(format!(r#"mklink /J "{link}" "{target}""#))
        .output()
    {
        Ok(output) => output,
        // No `cmd.exe` on this host, so `mklink` is unreachable: the fixture
        // cannot be built, and the caller reports a skip.
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        // Every other spawn error is a real fault, not unavailability.
        Err(err) => {
            return Err(err).context("run 'cmd /C mklink /J' for the junction fixture");
        }
    };
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        output.status.success(),
        "create junction fixture {link} -> {target}: cmd exited with {}: {}",
        output.status,
        stderr.trim()
    );
    let opened = open_link_ambient(&link)?;
    Ok(Some(JunctionFixture {
        _temp: temp,
        dir,
        opened,
        link,
        target,
    }))
}

/// The default policy's handle *is* the reparse point, not its target.
///
/// This is the property that makes the same-handle judgement possible, and the
/// only test that fails if `FILE_FLAG_OPEN_REPARSE_POINT` stops being passed:
/// without it the open returns the target directory, whose attributes carry no
/// reparse bit, and `reject_reparse_point` would wave the junction through. The
/// integration test cannot detect that, because `std` reports a junction as a
/// symlink, so `metadata.is_file()` refuses the directory anyway. The
/// distinction only shows on the handle.
///
/// A junction is used rather than a symlink because it needs no privilege, and
/// it is created with `mklink /J`. Every step is a genuine filesystem
/// operation; none of it substitutes an ordinary file for the reparse point
/// under test.
///
/// The handle is taken through the ambient authority rather than through the
/// capability. `mklink /J` records an absolute target, and `cap_std` refuses to
/// resolve a reparse point whose destination leaves the capability —
/// `escape_attempt()`, reported as `PermissionDenied`. The default policy never
/// resolves it, because `FILE_FLAG_OPEN_REPARSE_POINT` makes the open return the
/// reparse point itself, so the difference between the policies is visible
/// exactly where it matters: on the handle the policy will judge. The opt-in
/// policy's follow is covered end to end by
/// `follow_symlinks_opt_in_reads_the_link_target` in the integration suite.
#[cfg(windows)]
#[test]
#[expect(
    clippy::print_stderr,
    reason = "test harness: an unavailable fixture must be visible in the captured test output instead of passing silently"
)]
fn the_default_handle_is_the_junction_not_its_target() -> Result<()> {
    let Some(fixture) = junction_fixture()? else {
        // No `cmd.exe` on this host, so `mklink` is unreachable and the junction
        // cannot be built. Say so rather than passing silently: a green run that
        // quietly skipped its subject would hide the same regression on a host
        // that can build one.
        eprintln!("skipped: this host has no cmd.exe to create a junction fixture");
        return Ok(());
    };
    let JunctionFixture {
        _temp,
        dir,
        opened,
        link,
        target,
    } = fixture;

    // The handle the fixture took must be the reparse point itself.
    let metadata = opened
        .metadata()
        .context("read metadata from the default-policy handle")?;
    ensure!(
        is_prohibited_reparse_point(metadata.file_attributes()),
        "the default handle must be the reparse point, so the policy has \
         something to refuse; a handle to the target directory means \
         FILE_FLAG_OPEN_REPARSE_POINT was not passed"
    );
    ensure!(
        reject_reparse_point(&metadata, &link).is_err(),
        "the default policy must refuse the junction it just opened"
    );

    // The target directory is not itself a reparse point, which is what makes
    // the refusal above a decision about the link rather than about directories
    // in general. Without this the two checks could both pass on a host that
    // stamped the attribute on an ordinary directory.
    let target_metadata = dir
        .metadata(Utf8Path::new(TARGET))
        .context("read metadata for the junction fixture target directory")?;
    ensure!(
        !is_prohibited_reparse_point(target_metadata.file_attributes()),
        "the target directory must be an ordinary directory, not a reparse \
         point; otherwise the refusal above proves nothing about the link"
    );
    ensure!(
        reject_reparse_point(&target_metadata, &target).is_ok(),
        "the policy must not refuse the ordinary directory the link points at"
    );
    Ok(())
}
