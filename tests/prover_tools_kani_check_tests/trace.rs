//! Detect filesystem and network attempts in a syscall trace.
//!
//! A sandbox that refuses a write or a connection cannot, on its own, show
//! that either was attempted: a caller that discards the error (`2>/dev/null`,
//! `|| true`) leaves the run looking identical to one that never tried. The
//! kernel's refusal is visible only in the trace, so the isolation case runs
//! the query under `strace` and reads the attempts back from the log. This
//! module turns that log into findings. It reports attempts, not successes,
//! which is what makes a silently failed write detectable.
//!
//! The reading is deliberately literal: a syscall the trace does not contain is
//! a syscall that was never made.

/// The syscall classes the sandbox traces.
///
/// The filter selects classes rather than naming calls one by one. `%file` is
/// every call that takes a path, which is every call that can read, create,
/// remove, rename, re-time or resize one; `%network` is every call that can
/// carry a socket address. Naming calls individually would be wrong on half the
/// supported architectures: `mkdir` has no syscall of its own on aarch64, where
/// the same operation is `mkdirat`, and `strace` refuses to start when a
/// qualifier names a call the current architecture does not have. Covering the
/// classes means the trace cannot omit a call a reader entry expects, and the
/// reader still decides which of the calls it sees are attempts.
///
/// Only the sandbox on Linux runs `strace`, so the constant is compiled only
/// there. A non-Linux Unix build would otherwise carry it unused, which
/// `-D warnings` turns into a test failure.
#[cfg(target_os = "linux")]
pub(crate) const TRACE_SYSCALLS: &str = "%file,%network";

/// Syscalls that open a path, with their flags in the arguments.
///
/// `openat2` carries the same flag text as `openat`, nested inside the
/// `open_how` structure, so the same reading applies to all three.
const OPEN_SYSCALLS: [&str; 3] = ["open", "openat", "openat2"];

/// Syscalls that can address the network by carrying a socket address.
///
/// `sendmmsg` carries one address per message rather than one for the call, so
/// the arguments still name `AF_INET` or `AF_INET6` whenever any message does.
const NETWORK_SYSCALLS: [&str; 4] = ["connect", "sendto", "sendmsg", "sendmmsg"];

/// Open flags that ask for the path to be written rather than read.
const WRITE_FLAGS: [&str; 5] = ["O_WRONLY", "O_RDWR", "O_CREAT", "O_TRUNC", "O_APPEND"];

/// Syscalls that create, remove, rename, re-time or resize a path outright,
/// together with the calls that change a path's ownership or extended
/// attributes.
///
/// `creat` is here rather than with the open family because it takes no flag
/// argument: creating the file is the whole of what it does. The descriptor
/// forms (`fchmod`, `fchown`, `ftruncate`, `fsetxattr`) are named for
/// completeness even though `%file` does not report them; each needs a
/// descriptor, so the write-intent open that produced it is what the trace
/// shows.
///
/// The re-time calls are listed individually because there is no common
/// prefix `strace` reports: `utimensat` supersedes `utimes` and `futimesat`,
/// and `utime` survives on older architectures. The same holds for the
/// extended-attribute pair: writing one and removing one are both mutations of
/// a path's metadata, and either can be suppressed by a caller that discards
/// the failure.
const MUTATING_SYSCALLS: [&str; 34] = [
    "creat",
    "mkdir",
    "mkdirat",
    "unlink",
    "unlinkat",
    "rmdir",
    "rename",
    "renameat",
    "renameat2",
    "link",
    "linkat",
    "symlink",
    "symlinkat",
    "mknod",
    "mknodat",
    "utime",
    "utimes",
    "futimesat",
    "utimensat",
    "truncate",
    "ftruncate",
    "chmod",
    "fchmod",
    "fchmodat",
    "chown",
    "lchown",
    "fchown",
    "fchownat",
    "setxattr",
    "lsetxattr",
    "fsetxattr",
    "removexattr",
    "lremovexattr",
    "fremovexattr",
];

/// The character devices a program may legitimately open for writing.
///
/// A diagnostic goes to a stream that is occasionally `/dev/null`, and the
/// terminal is opened by some shells. These are not filesystem writes, and
/// treating them as such would report every run rather than the suspicious one.
const DEVICE_PATHS: [&str; 3] = ["/dev/null", "/dev/tty", "/dev/full"];

/// Return every filesystem write or network connection the trace attempted.
///
/// Both a completed call and a refused one are reported, because the refusal is
/// exactly what a caller can discard. Calls that were interrupted mid-line by
/// `strace -f` are reported from the line that carries their arguments; the
/// matching resumed line has no argument list and adds nothing.
pub(crate) fn attempts(log: &str) -> Vec<String> {
    log.lines().filter_map(finding).collect()
}

/// Return the finding a single `strace` line describes, if it describes one.
///
/// The three families are tried in turn so each one's condition stays small
/// enough to read on its own; a line matches at most one of them.
fn finding(line: &str) -> Option<String> {
    let (prefix, arguments) = line.split_once('(')?;
    let name = prefix.split_whitespace().last()?;
    path_change(name)
        .or_else(|| network_call(name, arguments))
        .or_else(|| write_open(name, arguments))
}

/// Report a syscall that creates, removes, renames, re-times or resizes a path.
fn path_change(name: &str) -> Option<String> {
    MUTATING_SYSCALLS
        .contains(&name)
        .then(|| format!("`{name}` changes a path"))
}

/// Report a call that addresses the network.
///
/// A Unix-domain address is local and accepted: only `AF_INET` and `AF_INET6`
/// leave the host. `sendto` and `sendmsg` on a connected socket carry no
/// address at all, but the `connect` that made the socket connected is itself
/// in the traced set, so the attempt is still reported.
fn network_call(name: &str, arguments: &str) -> Option<String> {
    let is_network = NETWORK_SYSCALLS.contains(&name) && reaches_network(arguments);
    is_network.then(|| format!("`{name}` to a network address: {arguments}"))
}

/// Report whether a socket address names another host.
///
/// The address family is read from the selectors the call carries, not from a
/// quoted path: a Unix socket at `/run/AF_INET` is still a local socket.
fn reaches_network(arguments: &str) -> bool {
    let selectors = unquoted(arguments);
    selectors.contains("AF_INET") || selectors.contains("AF_INET6")
}

/// Report an open that asks to write a path other than a character device.
fn write_open(name: &str, arguments: &str) -> Option<String> {
    let writes = OPEN_SYSCALLS.contains(&name) && requests_write(arguments);
    (writes && !is_device(arguments)).then(|| format!("a write-intent `{name}`: {arguments}"))
}

/// Report whether the open flags ask for the path to be written.
///
/// The flags are read from everything the call carries *except* its quoted
/// runs, because a path can be named after a flag. A read-only open of
/// `/repo/O_CREAT.log` is a read; searching the whole argument list would call
/// it a write and report a query that never asked to write anything.
fn requests_write(arguments: &str) -> bool {
    let flags = unquoted(arguments);
    WRITE_FLAGS.iter().any(|flag| flags.contains(flag))
}

/// Return `text` with every quoted run removed.
///
/// `strace` quotes the path in each call that takes one, so a name inside a
/// quoted run is path text rather than a flag or an address selector. An
/// unterminated quote — a line `strace` truncated mid-write — ends the retained
/// text, since nothing after it can be read as an argument. A backslash inside
/// a quoted run escapes the next character, as `strace` writes `\"` in a path.
fn unquoted(text: &str) -> String {
    let (mut kept, mut quoted, mut chars) = (String::new(), false, text.chars());
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (_, '"') => quoted = !quoted,
            (true, '\\') => drop(chars.next()),
            (false, _) => kept.push(c),
            (true, _) => {}
        }
    }
    kept
}

/// Report whether the path is one of the character devices a caller writes to.
///
/// The first quoted run is compared whole: `/dev/null.log` is an ordinary file,
/// and so is `/tmp/\"/dev/null`, whose escaped quote a substring test would
/// read as the opening quote of the device.
fn is_device(arguments: &str) -> bool {
    let (_, rest) = arguments.split_once('"').unwrap_or_default();
    let mut escaped = false;
    for (index, c) in rest.char_indices() {
        match c {
            '\\' if !escaped => escaped = true,
            '"' if !escaped => {
                return rest
                    .get(..index)
                    .is_some_and(|path| DEVICE_PATHS.contains(&path));
            }
            _ => escaped = false,
        }
    }
    false
}

#[cfg(test)]
#[path = "trace/tests.rs"]
mod tests;
