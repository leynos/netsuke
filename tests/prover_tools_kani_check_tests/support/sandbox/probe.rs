//! Ask the host whether it can build the isolation sandbox, and report why not.
//!
//! Creating the namespaces does not prove the sandbox can build its chroot: a
//! host may allow `unshare` and still deny a bind mount, a read-only remount, a
//! procfs mount, or `chroot`. Probing the same sequence on a throwaway directory
//! turns that into a reported prerequisite skip instead of a failure inside the
//! real script. This module owns that probe, the skip it produces, and the
//! tracer check, leaving [`super`] to the scripts it gates.

use super::super::InstalledKani;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

/// Environment variable that turns an unavailable isolation sandbox into a
/// failure instead of a reported skip.
const SANDBOX_STRICT_ENV: &str = "NETSUKE_KANI_CHECK_SANDBOX_STRICT";

/// The capability probe run inside the namespace before the real sandbox.
///
/// The probe binds the same `/etc`, repository, fixture, verifier-directory and
/// Kani-home paths the sandbox will use, because a non-recursive bind of a
/// directory that contains an inherited submount can fail even when the simpler
/// binds succeed. Each stage has its own exit status so the reason names the
/// operation that was refused.
const PROBE: &str = r#"set -u
root=$1; repo=$2; fixture=$3; trace=$4; verifier=$5; kani_home=$6
mkdir -p "$root/usr" "$root/etc" "$root/proc" "$root/dev" "$root/opt/verifier" "$root$repo" "$root$fixture" "$root/var/trace" "$trace" || exit 90
if [ -n "$kani_home" ]; then
  mkdir -p "$root$kani_home" || exit 90
fi
touch "$root/dev/null" || exit 90
ln -s usr/bin "$root/bin" || exit 90
ln -s usr/sbin "$root/sbin" || exit 90
ln -s usr/lib "$root/lib" || exit 90
ln -s usr/lib64 "$root/lib64" || exit 90
mount --make-rprivate / || true
mount --bind "$root" "$root" && mount -o remount,ro,bind "$root" || exit 91
mount --bind /usr "$root/usr" && mount -o remount,ro,bind "$root/usr" || exit 92
mount --bind /etc "$root/etc" && mount -o remount,ro,bind "$root/etc" || exit 93
mount --bind "$repo" "$root$repo" && mount -o remount,ro,bind "$root$repo" || exit 94
mount --bind "$fixture" "$root$fixture" || exit 95
mount -o remount,ro,bind "$root$fixture" || exit 96
mount -t proc proc "$root/proc" || exit 97
mount --bind /dev/null "$root/dev/null" || exit 98
mount --bind "$trace" "$root/var/trace" || exit 99
if [ -n "$verifier" ]; then
  mount --bind "$verifier" "$root/opt/verifier" \
    && mount -o remount,ro,bind "$root/opt/verifier" || exit 101
fi
if [ -n "$kani_home" ]; then
  mount --bind "$kani_home" "$root$kani_home" \
    && mount -o remount,ro,bind "$root$kani_home" || exit 102
fi
chroot "$root" /bin/sh -c 'exit 0' || exit 100
"#;

/// Name the sandbox operation a probe exit status stands for.
fn probe_stage(status: Option<i32>) -> &'static str {
    match status {
        Some(90) => "create the probe directories",
        Some(91) => "bind and remount the chroot root read-only",
        Some(92) => "bind and remount /usr read-only",
        Some(93) => "bind and remount /etc read-only",
        Some(94) => "bind and remount the repository read-only",
        Some(95) => "bind the fixture directory",
        Some(96) => "remount the fixture directory read-only",
        Some(97) => "mount a fresh procfs",
        Some(98) => "bind /dev/null into the probe root",
        Some(99) => "bind the trace directory",
        Some(100) => "chroot into the probe root",
        Some(101) => "bind and remount the installed verifier read-only",
        Some(102) => "bind and remount the Kani home read-only",
        _ => "run the capability probe",
    }
}

/// Report why the isolation sandbox cannot run, if it cannot.
///
/// A host without user namespaces cannot demonstrate the blocked state, and a
/// host that creates them may still refuse one of the mount operations the
/// chroot needs. The probe asks for the same namespace set the sandbox uses,
/// including the process namespace that `mount -t proc` requires, and then
/// performs the same binds — `/usr`, `/etc`, the repository, the fixture and,
/// when one is named, the installed verifier's directory and its Kani home —
/// the same read-only remounts, the procfs mount, and `chroot` on a throwaway
/// directory, so a host that can do only part of it is reported as unavailable
/// rather than failing later inside the script. The case reports that as a
/// skip, which strict mode turns back into a failure so a lane that requires
/// the boundary test cannot pass without exercising it.
pub(crate) fn unavailable(
    repo: &Path,
    fixture: &Path,
    installed: Option<&InstalledKani>,
) -> Option<String> {
    let root = match tempdir() {
        Ok(root) => root,
        Err(error) => return Some(format!("cannot create a sandbox probe directory: {error}")),
    };
    let trace = match tempdir() {
        Ok(trace) => trace,
        Err(error) => return Some(format!("cannot create a sandbox probe directory: {error}")),
    };
    let (verifier, kani_home) = installed.map_or_else(
        || (String::new(), String::new()),
        |verifier| {
            (
                verifier.directory().display().to_string(),
                verifier.home.display().to_string(),
            )
        },
    );
    let probe = Command::new("unshare")
        .args([
            "--user",
            "--map-root-user",
            "--net",
            "--mount",
            "--pid",
            "--fork",
            "--",
            "sh",
            "-c",
            PROBE,
            "sh",
        ])
        .arg(root.path())
        .arg(repo)
        .arg(fixture)
        .arg(trace.path())
        .arg(verifier)
        .arg(kani_home)
        .output();
    match probe {
        Ok(output) if output.status.success() => None,
        Ok(output) => Some(format!(
            "the isolation sandbox cannot {}: {}",
            probe_stage(output.status.code()),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(error) => Some(format!("unshare is not runnable: {error}")),
    }
}

/// Report whether a syscall tracer is installed.
///
/// The read-only mounts can only show that a write did not succeed. Showing
/// that none was attempted — which is the contract — needs the trace, so a host
/// without `strace` cannot exercise the boundary's attempt-detection half.
pub(crate) fn tracer_available() -> bool {
    Command::new("strace")
        .arg("-V")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Report a skipped isolation case, unless strict mode forbids skipping.
///
/// # Errors
///
/// Returns an error when [`SANDBOX_STRICT_ENV`] is `1`, so a lane that requires
/// the boundary test fails rather than silently passing on a host that could
/// not run it.
#[expect(
    clippy::disallowed_methods,
    reason = "this is the single site that reads the strict-mode switch; the \
              test binary has no injected environment to consult"
)]
#[expect(
    clippy::print_stderr,
    reason = "a skipped prerequisite must be visible in the captured test output"
)]
pub(crate) fn skip(reason: &str) -> anyhow::Result<()> {
    let strict = std::env::var_os(SANDBOX_STRICT_ENV).is_some_and(|value| value == "1");
    anyhow::ensure!(
        !strict,
        "the kani-check isolation sandbox could not be exercised: {reason} \
         (strict mode requires the boundary test to run for real)"
    );
    eprintln!("skipped: kani-check isolation — {reason}");
    Ok(())
}
