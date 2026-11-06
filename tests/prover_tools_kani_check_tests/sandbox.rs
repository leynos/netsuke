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
/// allowed to read is then bound read-only on top of that root. Nothing else
/// exists inside the chroot, so the inherited home directory and the temporary
/// directories are not merely outside an allowlist but absent.
pub(super) fn script(paths: &SandboxPaths<'_>) -> String {
    format!(
        r#"set -u
root={chroot}
repo={repo}
fixture={fixture}
mkdir -p "$root/usr" "$root/etc" "$root/proc" "$root$repo" "$root$fixture" || exit 90
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
probe home "$HOME/sandbox-probe"
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

/// Report why the isolation sandbox cannot run, if it cannot.
///
/// A host without user namespaces cannot demonstrate the blocked state. The
/// probe asks for the same namespace set the sandbox uses, including the
/// process namespace that `mount -t proc` requires, so a host that can create
/// only some of them is reported as unavailable rather than failing later
/// inside the script. The case reports that as a skip, which strict mode turns
/// back into a failure so a lane that requires the boundary test cannot pass
/// without exercising it.
pub(crate) fn unavailable() -> Option<String> {
    let probe = Command::new("unshare")
        .args([
            "--user",
            "--map-root-user",
            "--net",
            "--mount",
            "--pid",
            "--fork",
            "--",
            "true",
        ])
        .output();
    match probe {
        Ok(output) if output.status.success() => None,
        Ok(output) => Some(format!(
            "unshare could not create an isolation namespace: {}",
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
