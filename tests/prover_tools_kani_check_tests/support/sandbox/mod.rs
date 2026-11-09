//! The read-only chroot that isolates the `kani-check` boundary test.
//!
//! The sandbox is a pair of shell scripts: an outer one that assembles a
//! chroot from read-only bind mounts inside a fresh set of namespaces, and an
//! inner one that verifies each restriction took effect before running the
//! real recipe. Keeping them here leaves `support` to the fixture plumbing and
//! keeps each file inside the module size cap.

use super::{InstalledKani, shell_quote};
use std::path::Path;

mod probe;

pub(crate) use probe::{skip, tracer_available, unavailable};

/// Paths the isolation sandbox script interpolates.
///
/// Grouping them keeps the script builder's signature small, and makes the
/// provenance of each interpolation explicit at the call site.
pub(super) struct SandboxPaths<'a> {
    pub(super) repo: &'a Path,
    pub(super) fixture: &'a Path,
    pub(super) chroot: &'a Path,
    pub(super) kani: &'a Path,
    pub(super) control: &'a Path,
    pub(super) version_file: &'a Path,
    pub(super) poison: &'a Path,
    pub(super) trace: &'a Path,
    /// A makefile whose recipe lines attempt suppressed writes, so the boundary
    /// shows it would notice an attempt made by the recipe and not only by the
    /// verifier the recipe runs.
    pub(super) dirty: &'a Path,
    /// The installed verifier to exercise inside the boundary, when the caller
    /// named one. Its binary is exposed read-only at `/opt/verifier`, and its
    /// Kani home read-only at its own path.
    pub(super) installed: Option<&'a InstalledKani>,
}

/// The installed verifier as the sandbox exposes and runs it.
///
/// Binding the whole cargo home would expose more of the caller's environment
/// than the boundary needs, so only the directory holding the binary is bound,
/// and only read-only. The verifier's shared libraries come from `/usr`, which
/// the sandbox already binds. Its Kani home is bound separately and at its own
/// path, because the launcher looks for the payload there and a home it cannot
/// read starts a first-time setup instead of answering.
struct InstalledVerifier<'a> {
    directory: &'a Path,
    name: &'a str,
    home: &'a Path,
}

impl<'a> InstalledVerifier<'a> {
    /// Describe the verifier a path names, if one was supplied.
    ///
    /// A binary without a parent cannot be bound, so it is reported as absent;
    /// the caller validates the path before reaching here.
    fn new(installed: Option<&'a InstalledKani>) -> Option<Self> {
        let verifier = installed?;
        let name = verifier.binary.file_name()?.to_str()?;
        Some(Self {
            directory: verifier.directory(),
            name,
            home: &verifier.home,
        })
    }

    /// Return the mount line that places the verifier in the chroot.
    fn mount(&self) -> String {
        format!(
            concat!(
                "mount --bind {directory} \"$root/opt/verifier\" && ",
                "mount -o remount,ro,bind \"$root/opt/verifier\" || exit 101\n",
            ),
            directory = shell_quote(self.directory)
        )
    }

    /// Return the argument naming the verifier inside the chroot.
    fn argument(&self) -> String {
        shell_quote(&Path::new("/opt/verifier").join(self.name))
    }
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
///
/// The one writable filesystem mount is the trace directory, which exists so
/// `strace` can record what the query attempted. It is bound rather than
/// created inside the chroot, so its contents survive on the host for the
/// caller to read, and the recipe is never told about it. `/dev/null` is bound
/// separately: programs and shells write there unconditionally, and its absence
/// makes a shell abort a redirection rather than report a refused write.
///
/// When the caller names an installed verifier its directory and its Kani home
/// are bound read-only too, each at its own host path. The home has to exist
/// before the chroot root is remounted, so it is created in the same pass as
/// the other paths the query is allowed to read.
pub(super) fn script(paths: &SandboxPaths<'_>) -> String {
    let installed = InstalledVerifier::new(paths.installed);
    let kani_home = installed
        .as_ref()
        .map_or_else(|| "''".to_owned(), |verifier| shell_quote(verifier.home));
    let install_mount = installed
        .as_ref()
        .map_or_else(String::new, InstalledVerifier::mount);
    let install_argument = installed
        .as_ref()
        .map_or_else(|| "''".to_owned(), InstalledVerifier::argument);
    let body = format!(
        r#"set -u
root={chroot}
repo={repo}
fixture={fixture}
trace={trace}
kani_home={kani_home}
mkdir -p "$root/usr" "$root/etc" "$root/proc" "$root/dev" "$root/tmp" "$root/opt/verifier" "$root/var/trace" "$root$repo" "$root$fixture" || exit 90
if [ -n "${{HOME:-}}" ]; then
  mkdir -p "$root$HOME" || exit 90
fi
if [ -n "$kani_home" ]; then
  mkdir -p "$root$kani_home" || exit 90
fi
touch "$root/dev/null" || exit 90
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
mount --bind /dev/null "$root/dev/null" || exit 98
mount --bind "$trace" "$root/var/trace" || exit 99
if [ -n "$kani_home" ]; then
  mount --bind "$kani_home" "$root$kani_home" && mount -o remount,ro,bind "$root$kani_home" || exit 102
fi
{install_mount}chroot "$root" /bin/sh -c '{inner}' sh "$repo" "$fixture" {kani} {control} {version_file} {poison} /var/trace {install_argument} "$kani_home" {dirty}
"#,
        chroot = shell_quote(paths.chroot),
        repo = shell_quote(paths.repo),
        fixture = shell_quote(paths.fixture),
        kani = shell_quote(paths.kani),
        control = shell_quote(paths.control),
        version_file = shell_quote(paths.version_file),
        poison = shell_quote(paths.poison),
        trace = shell_quote(paths.trace),
        dirty = shell_quote(paths.dirty),
        inner = INNER,
    );
    // The syscall set is the reader's, not the script's: a trace that omits a
    // call the reader looks for would report a clean run it never saw.
    body.replace("__TRACE_SYSCALLS__", crate::trace::TRACE_SYSCALLS)
}

/// The script run inside the chroot: probe the restrictions, then query.
///
/// The single-quoted literal the parent script passes to `sh -c` must not
/// contain an apostrophe, so this uses only double quotes. `control` is a
/// deliberately writable procfs entry: it proves the probe reports a plain
/// failure when the path really is protected, rather than reporting every path
/// as blocked because redirection is broken.
///
/// Three traced runs follow the probes: the real query, a control whose
/// verifier attempts a suppressed write and a suppressed connection, and a
/// control recipe whose own lines attempt suppressed writes. The reader must
/// find nothing in the first trace and the expected attempts in the other two,
/// which is how the boundary proves it would notice an attempt rather than only
/// a success. When the caller names an installed verifier, a fourth traced query
/// runs that binary through the same recipe, so the real executable is
/// exercised inside the boundary and not only the fixture's stub; that query
/// also points `KANI_HOME` at the bound payload, so the launcher answers rather
/// than installing.
///
/// The trace wraps `make`, not the verifier binary, so every process the recipe
/// starts is followed, and `makefile` names the makefile to run: the shipped
/// one for three of the runs, the fixture's dirty recipe for the control that
/// must be reported. The query's own trace shows the wrapping works: the
/// recipe's `cat` of the pin file appears in it, which is why the boundary
/// covers the recipe's commands and not just whatever those commands invoke.
const INNER: &str = r#"set -u
LC_ALL=C
export LC_ALL
repo=$1; fixture=$2; kani=$3; control=$4; version_file=$5; poison=$6; trace=$7; installed=$8; kani_home=$9; dirty=${10}
traced() {
  out=$1; verifier=$2; pin=${3:-$version_file}; makefile=${4:-Makefile}
  strace -f -qq -e "trace=__TRACE_SYSCALLS__" \
    -o "$trace/$out" make --no-print-directory -f "$makefile" kani-check \
    KANI="$verifier" KANI_VERSION_FILE="$pin" \
    PROVER_TOOLS="$poison" UV="$poison"
}
cd "$repo" || exit 90
# The launcher resolves its payload from KANI_HOME and starts a first-time
# setup when the home is missing, so the query is pointed at the bound home.
if [ -n "$kani_home" ]; then
  KANI_HOME="$kani_home"
  export KANI_HOME
fi
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
traced query.strace "$kani"
rc=$?
traced control.strace "$control"
control_rc=$?
traced dirty.strace "$kani" "$version_file" "$dirty"
dirty_rc=$?
if [ -n "$installed" ]; then
  traced installed.strace "$installed" "$repo/tools/kani/VERSION"
  installed_rc=$?
else
  installed_rc=unset
fi
echo "make-exit=$rc"
echo "control-exit=$control_rc"
echo "dirty-exit=$dirty_rc"
echo "installed-exit=$installed_rc"
exit "$rc"
"#;
