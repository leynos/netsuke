//! The read-only chroot that isolates the `kani-check` boundary test.
//!
//! The sandbox is a pair of shell scripts: an outer one that assembles a
//! chroot from read-only bind mounts inside a fresh set of namespaces, and an
//! inner one that verifies each restriction took effect before running the
//! real recipe. Keeping them here leaves `support` to the fixture plumbing and
//! keeps each file inside the module size cap.

use super::shell_quote;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

/// Environment variable that turns an unavailable isolation sandbox into a
/// failure instead of a reported skip.
pub(crate) const SANDBOX_STRICT_ENV: &str = "NETSUKE_KANI_CHECK_SANDBOX_STRICT";

/// Paths the isolation sandbox script interpolates.
///
/// Grouping them keeps the script builder's signature small, and makes the
/// provenance of each interpolation explicit at the call site.
pub(super) struct SandboxPaths<'a> {
    pub(super) repo: &'a Path,
    pub(super) fixture: &'a Path,
    pub(super) chroot: &'a Path,
    pub(super) kani: &'a Path,
    pub(super) version_file: &'a Path,
    pub(super) poison: &'a Path,
}

/// Build the shell script that applies the restrictions and runs the query.
///
/// Each restriction is asserted, not merely attempted: a sandbox that fails to
/// take effect would otherwise make the boundary test pass vacuously. The
/// chroot's own root is a self-bind remounted read-only, because `/` itself
/// cannot be remounted from inside a user namespace; everything the query is
/// allowed to read is then bound read-only on top of that root. `/tmp` and the
/// caller's home directory are created inside the chroot before it is made
/// read-only, so they exist and are proven unwritable rather than merely
/// absent; the home directory is created only when the caller has one, and the
/// probe reports it as absent otherwise.
pub(super) fn script(paths: &SandboxPaths<'_>) -> String {
    format!(
        r#"set -u
root={chroot}
repo={repo}
fixture={fixture}
mkdir -p "$root/usr" "$root/etc" "$root/proc" "$root/tmp" "$root$repo" "$root$fixture" || exit 90
if [ -n "${{HOME:-}}" ]; then
  mkdir -p "$root$HOME" || exit 90
fi
ln -s usr/bin "$root/bin"
ln -s usr/sbin "$root/sbin"
ln -s usr/lib "$root/lib"
ln -s usr/lib64 "$root/lib64"
mount --make-rprivate / || true
mount --bind "$root" "$root" && mount -o remount,ro,bind "$root" || exit 91
mount --bind /usr "$root/usr" && mount -o remount,ro,bind "$root/usr" || exit 92
mount --bind /etc "$root/etc" && mount -o remount,ro,bind "$root/etc" || exit 93
mount --bind "$repo" "$root$repo" && mount -o remount,ro,bind "$root$repo" || exit 94
mount --bind "$fixture" "$root$fixture" || exit 95
mount -o remount,ro,bind "$root$fixture" || exit 96
mount -t proc proc "$root/proc" || exit 97
exec chroot "$root" /bin/sh -c '{inner}' sh "$repo" "$fixture" {kani} {version_file} {poison}
"#,
        chroot = shell_quote(paths.chroot),
        repo = shell_quote(paths.repo),
        fixture = shell_quote(paths.fixture),
        kani = shell_quote(paths.kani),
        version_file = shell_quote(paths.version_file),
        poison = shell_quote(paths.poison),
        inner = INNER,
    )
}

/// The script run inside the chroot: probe the restrictions, then query.
///
/// The single-quoted literal the parent script passes to `sh -c` must not
/// contain an apostrophe, so this uses only double quotes. `control` is a
/// deliberately writable procfs entry: it proves the probe reports a plain
/// failure when the path really is protected, rather than reporting every path
/// as blocked because redirection is broken.
const INNER: &str = r#"set -u
LC_ALL=C
export LC_ALL
repo=$1; fixture=$2; kani=$3; version_file=$4; poison=$5
cd "$repo" || exit 90
probe() {
  label=$1; path=$2
  if err=$( ( : > "$path" ) 2>&1 ); then
    echo "$label=writable"
    return 0
  fi
  case $err in
    *"Read-only file system"*) echo "$label=read-only" ;;
    *"No such file or directory"*) echo "$label=absent" ;;
    *) echo "$label=other" ;;
  esac
}
probe control /proc/self/comm
probe repo "$repo/sandbox-probe"
probe fixture "$fixture/sandbox-probe"
probe rootdir /sandbox-probe
probe usr /usr/sandbox-probe
probe etc /etc/sandbox-probe
probe tmp /tmp/sandbox-probe
if [ -n "${HOME:-}" ]; then
  probe home "$HOME/sandbox-probe"
else
  echo "home=absent"
fi
echo "ifaces=$(cut -d: -f1 /proc/net/dev | tail -n +3 | tr -d " " | tr "\n" ",")"
if resolved=$(timeout 5 getent hosts example.com 2>&1); then
  echo "dns=resolved"
else
  echo "dns=blocked"
fi
make --no-print-directory kani-check \
  KANI="$kani" KANI_VERSION_FILE="$version_file" \
  PROVER_TOOLS="$poison" UV="$poison"
rc=$?
echo "make-exit=$rc"
exit "$rc"
"#;

/// The capability probe run inside the namespace before the real sandbox.
///
/// Creating the namespaces does not prove the sandbox can build its chroot: a
/// host may allow `unshare` and still deny a bind mount, a read-only remount, a
/// procfs mount, or `chroot`. Probing the same sequence on a throwaway
/// directory turns that into a reported prerequisite skip instead of a failure
/// inside the real script. Each stage has its own exit status so the reason
/// names the operation that was refused.
const PROBE: &str = r#"set -u
root=$1
mkdir -p "$root/usr" "$root/proc" || exit 90
ln -s usr/bin "$root/bin" || exit 90
ln -s usr/sbin "$root/sbin" || exit 90
ln -s usr/lib "$root/lib" || exit 90
ln -s usr/lib64 "$root/lib64" || exit 90
mount --make-rprivate / || true
mount --bind "$root" "$root" && mount -o remount,ro,bind "$root" || exit 91
mount --bind /usr "$root/usr" && mount -o remount,ro,bind "$root/usr" || exit 92
mount -t proc proc "$root/proc" || exit 93
chroot "$root" /bin/sh -c 'exit 0' || exit 94
"#;

/// Name the sandbox operation a probe exit status stands for.
fn probe_stage(status: Option<i32>) -> &'static str {
    match status {
        Some(90) => "create the probe directories",
        Some(91) => "bind and remount the chroot root read-only",
        Some(92) => "bind and remount /usr read-only",
        Some(93) => "mount a fresh procfs",
        Some(94) => "chroot into the probe root",
        _ => "run the capability probe",
    }
}

/// Report why the isolation sandbox cannot run, if it cannot.
///
/// A host without user namespaces cannot demonstrate the blocked state, and a
/// host that creates them may still refuse one of the mount operations the
/// chroot needs. The probe asks for the same namespace set the sandbox uses,
/// including the process namespace that `mount -t proc` requires, and then
/// performs the same bind, read-only remount, procfs mount, and `chroot` on a
/// throwaway directory, so a host that can do only part of it is reported as
/// unavailable rather than failing later inside the script. The case reports
/// that as a skip, which strict mode turns back into a failure so a lane that
/// requires the boundary test cannot pass without exercising it.
pub(crate) fn unavailable() -> Option<String> {
    let root = match tempdir() {
        Ok(root) => root,
        Err(error) => return Some(format!("cannot create a sandbox probe directory: {error}")),
    };
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
