//! Directory-alias fixture helpers for the ambient filesystem boundary.
//!
//! Aliases are the one fixture operation whose platform support differs in
//! kind rather than in spelling: Unix creates symbolic links, Windows needs a
//! junction reached through the `cmd.exe` `mklink` built-in. Keeping them in
//! their own module lets the general-purpose helpers in
//! [`crate::fs`] stay platform-neutral.
//!
//! Scope and reuse policy: test fixtures and assertions only, reached through
//! the [`crate::fs`] re-exports so callers keep a single ambient boundary. The
//! module path is `test_support::fs::links`, a descendant of the sanctioned
//! `test_support::fs` exclusion, so it inherits that policy rather than adding
//! a new exemption.

use std::fs;
use std::io;
use std::path::Path;

/// Create a symbolic link at `link` pointing to `target` on Unix.
///
/// # Errors
///
/// Propagates the underlying symbolic-link creation failure.
///
/// # Examples
///
/// ```
/// let dir = tempfile::tempdir().expect("create tempdir");
/// let target = dir.path().join("target");
/// test_support::fs::create_dir(&target).expect("create target");
/// test_support::fs::symlink(&target, dir.path().join("link")).expect("create symlink");
/// ```
#[cfg(unix)]
pub fn symlink(target: impl AsRef<Path>, link: impl AsRef<Path>) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

/// Return `true` when `path`'s directory entry is a symbolic link.
///
/// Unlike [`crate::fs::exists`], this does not follow the link, so it reports
/// the entry itself rather than its target.
///
/// # Examples
///
/// ```
/// # #[cfg(unix)]
/// # {
/// let dir = tempfile::tempdir().expect("create tempdir");
/// let target = dir.path().join("target");
/// test_support::fs::write(&target, "contents").expect("write target");
/// let link = dir.path().join("link");
/// test_support::fs::symlink(&target, &link).expect("create symlink");
/// assert!(test_support::fs::is_symlink(&link));
/// assert!(!test_support::fs::is_symlink(&target));
/// # }
/// ```
#[must_use]
pub fn is_symlink(path: impl AsRef<Path>) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink())
}

/// Create a directory junction at `link` pointing to `target` on Windows.
///
/// Returns `Ok(false)` when this host has no `cmd.exe` to reach the `mklink`
/// built-in, so a caller can report a skip rather than a failure. A junction
/// needs no privilege, so a missing `cmd.exe` is the only unavailability that
/// is a skip; every other failure is a real fault.
///
/// # Errors
///
/// Returns an error when a path contains a quote and cannot be passed to
/// `cmd`, when `cmd` cannot be spawned for a reason other than its absence, or
/// when `mklink` reports failure.
///
/// # Examples
///
/// ```no_run
/// # #[cfg(windows)]
/// # {
/// let dir = tempfile::tempdir().expect("create tempdir");
/// let target = dir.path().join("target");
/// test_support::fs::create_dir(&target).expect("create target");
/// let link = dir.path().join("link");
/// if test_support::fs::junction(&target, &link).expect("create junction") {
///     assert!(test_support::fs::is_dir(&link));
/// }
/// # }
/// ```
#[cfg(windows)]
pub fn junction(target: impl AsRef<Path>, link: impl AsRef<Path>) -> io::Result<bool> {
    use std::os::windows::process::CommandExt as _;
    use std::process::Command;

    let target = target.as_ref();
    let link = link.as_ref();
    let target_text = target.to_string_lossy();
    let link_text = link.to_string_lossy();
    if target_text.contains('"') || link_text.contains('"') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a junction path contains a quote and cannot be passed to cmd",
        ));
    }
    // `mklink` is a `cmd` built-in, so it is reachable only through `cmd /C`.
    // `raw_arg` passes the line verbatim because `cmd` parses it itself.
    //
    // The paths travel in child environment variables rather than in the
    // command text: `cmd` expands `%NAME%` wherever it appears, so a fixture
    // directory legitimately named `%USERNAME%` would otherwise be rewritten
    // to the ambient value. Its expansion is a single pass, so a `%` that
    // arrives through a variable's value stays literal. `/V:OFF` keeps `!`
    // literal too, because delayed expansion is off for this child.
    let output = match Command::new("cmd")
        .arg("/V:OFF")
        .arg("/C")
        .raw_arg(r#"mklink /J "%NETSUKE_JUNCTION_LINK%" "%NETSUKE_JUNCTION_TARGET%""#)
        .env("NETSUKE_JUNCTION_LINK", &link_text)
        .env("NETSUKE_JUNCTION_TARGET", &target_text)
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if output.status.success() {
        return Ok(true);
    }
    Err(io::Error::other(format!(
        "mklink /J failed with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}
