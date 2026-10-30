//! Fixtures that create real platform file types inside a test workspace.
//!
//! A special-file policy test must create the requested file type or skip
//! because that file type is unavailable; it must never substitute a regular
//! file. Each builder therefore reports `Ok(None)` only for the documented
//! environmental unavailability, and propagates every other failure so a
//! policy that went unexercised is never reported as a pass.

#[cfg(windows)]
use anyhow::ensure;
use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Create the workspace's real file symlink, `<root>/link` -> `file`, and
/// report the link's path.
///
/// `Ok(None)` means this host cannot provide a file symlink at all: either
/// the platform has no symlink support, or Windows refused for want of
/// `SeCreateSymbolicLinkPrivilege` and Developer Mode — the environmental
/// `ERROR_PRIVILEGE_NOT_HELD` condition. Every other failure is a genuine
/// setup fault and propagates.
///
/// Callers skip their symlink-specific assertions on `Ok(None)`. They must
/// not fall back to a regular file: a special-file policy test must create
/// the requested file type or skip because that file type is unavailable.
/// It must not substitute a regular file.
///
/// # Errors
///
/// Returns the setup error when the platform can create symlinks but this
/// one was not created.
pub(crate) fn file_symlink_fixture(root: &Utf8Path) -> Result<Option<Utf8PathBuf>> {
    #[cfg(unix)]
    {
        let dir = Dir::open_ambient_dir(root, ambient_authority())
            .context("open filter workspace for the symlink fixture")?;
        dir.symlink("file", "link")
            .context("create fixture symlink 'link' -> 'file'")?;
        Ok(Some(root.join("link")))
    }
    #[cfg(windows)]
    {
        let link = root.join("link");
        match std::os::windows::fs::symlink_file("file", &link) {
            Ok(()) => Ok(Some(link)),
            Err(err) if is_symlink_privilege_error(&err) => Ok(None),
            Err(err) => Err(err).context("create fixture symlink 'link' -> 'file'"),
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = root;
        Ok(None)
    }
}

/// Whether `err` is Windows declining a symlink for want of
/// `SeCreateSymbolicLinkPrivilege` and Developer Mode.
///
/// Only that documented environmental condition reports "symlink
/// unavailable"; callers surface anything else as a setup failure. Matching
/// the raw status rather than `PermissionDenied` keeps an ACL denial on the
/// workspace visible instead of silently skipping the symlink assertions.
#[cfg(windows)]
pub(super) fn is_symlink_privilege_error(err: &std::io::Error) -> bool {
    /// Status `CreateSymbolicLink` reports when neither the privilege nor
    /// Developer Mode is available.
    const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;
    err.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD)
}

/// Create the workspace's real directory junction, `<root>/junc` ->
/// `<root>/junction_target`, and report the junction's path.
///
/// A junction is the directory-shaped reparse point this policy refuses.
/// Unlike a symlink it needs no privilege and no Developer Mode, so the
/// fixture is available on every ordinary Windows host; it is built with
/// `cmd /C mklink /J`, which is the only unprivileged route to one.
///
/// `Ok(None)` means this host has no `cmd.exe`, so the fixture cannot be
/// created at all. That is the only environmental unavailability this
/// fixture recognizes. Every other failure — including a non-zero `mklink`
/// exit — is a setup fault and propagates, because junction creation needs
/// no privilege and reparse points are supported on every filesystem this
/// suite runs on: skipping there would report green over a policy that went
/// unexercised.
///
/// Callers assert the entry really carries the reparse-point attribute
/// before rendering. They must not fall back to a plain directory: a
/// special-file policy test must create the requested file type or skip
/// because that file type is unavailable. It must not substitute one.
///
/// # Errors
///
/// Returns the setup error when `cmd` is present but the junction was not
/// created, quoting `mklink`'s own diagnostics.
#[cfg(windows)]
pub(crate) fn junction_fixture(root: &Utf8Path) -> Result<Option<Utf8PathBuf>> {
    use std::os::windows::process::CommandExt as _;
    use std::process::Command;

    /// Name of the junction the fixture creates.
    const LINK: &str = "junc";
    /// Name of the directory the junction points at.
    const TARGET: &str = "junction_target";

    let dir = Dir::open_ambient_dir(root, ambient_authority())
        .context("open filter workspace for the junction fixture")?;
    dir.create_dir(TARGET)
        .context("create the junction fixture target directory")?;
    let link = root.join(LINK);
    let target = root.join(TARGET);
    ensure!(
        !link.as_str().contains('"') && !target.as_str().contains('"'),
        "workspace path contains a quote and cannot be passed to cmd: \
             link {link}, target {target}"
    );

    // `mklink` is a `cmd` built-in, so it is reachable only through
    // `cmd /C`. `raw_arg` passes the command line verbatim because `cmd`
    // parses that line itself, while Rust's standard argument quoting
    // targets the C runtime's rules rather than `cmd`'s.
    let output = match Command::new("cmd")
        .arg("/C")
        .raw_arg(format!(r#"mklink /J "{link}" "{target}""#))
        .output()
    {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
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
    Ok(Some(link))
}
