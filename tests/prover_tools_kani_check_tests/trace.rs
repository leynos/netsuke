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
pub(crate) const TRACE_SYSCALLS: &str = "%file,%network";

/// Syscalls that open a path, with their flags in the arguments.
///
/// `openat2` carries the same flag text as `openat`, nested inside the
/// `open_how` structure, so the same reading applies to all three.
const OPEN_SYSCALLS: [&str; 3] = ["open", "openat", "openat2"];

/// Syscalls that can address the network by carrying a socket address.
const NETWORK_SYSCALLS: [&str; 3] = ["connect", "sendto", "sendmsg"];

/// Open flags that ask for the path to be written rather than read.
const WRITE_FLAGS: [&str; 5] = ["O_WRONLY", "O_RDWR", "O_CREAT", "O_TRUNC", "O_APPEND"];

/// Syscalls that create, remove, rename, re-time or resize a path outright.
///
/// `creat` is here rather than with the open family because it takes no flag
/// argument: creating the file is the whole of what it does. The descriptor
/// forms (`fchmod`, `fchown`, `ftruncate`) are named for completeness even
/// though `%file` does not report them; each needs a descriptor, so the
/// write-intent open that produced it is what the trace shows.
const MUTATING_SYSCALLS: [&str; 24] = [
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
    "utimensat",
    "truncate",
    "ftruncate",
    "chmod",
    "fchmod",
    "fchmodat",
    "chown",
    "fchown",
    "fchownat",
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
fn reaches_network(arguments: &str) -> bool {
    arguments.contains("AF_INET") || arguments.contains("AF_INET6")
}

/// Report an open that asks to write a path other than a character device.
fn write_open(name: &str, arguments: &str) -> Option<String> {
    let writes = OPEN_SYSCALLS.contains(&name) && requests_write(arguments);
    (writes && !is_device(arguments)).then(|| format!("a write-intent `{name}`: {arguments}"))
}

/// Report whether the open flags ask for the path to be written.
fn requests_write(arguments: &str) -> bool {
    WRITE_FLAGS.iter().any(|flag| arguments.contains(flag))
}

/// Report whether the path is one of the character devices a caller writes to.
fn is_device(arguments: &str) -> bool {
    DEVICE_PATHS.iter().any(|device| arguments.contains(device))
}

#[cfg(test)]
mod tests {
    //! Cases for the reader, so the oracle is known to fire rather than known
    //! to pass on today's log.

    use super::attempts;
    use rstest::rstest;

    #[rstest]
    #[case::suppressed_write(
        r#"7     openat(AT_FDCWD, "/repo/probe", O_WRONLY|O_CREAT|O_TRUNC, 0666) = -1 EROFS (Read-only file system)"#,
        "write-intent"
    )]
    #[case::appending_write(
        r#"7     openat(AT_FDCWD, "/tmp/log", O_WRONLY|O_APPEND) = -1 EROFS (Read-only file system)"#,
        "write-intent"
    )]
    #[case::create_only(r#"7     creat("/tmp/x", 0666) = -1 EROFS"#, "`creat`")]
    #[case::directory_creation(r#"7     mkdir("/tmp/x", 0777) = -1 EROFS"#, "`mkdir`")]
    #[case::directory_creation_at(
        r#"7     mkdirat(AT_FDCWD, "/tmp/x", 0777) = -1 EROFS"#,
        "`mkdirat`"
    )]
    #[case::directory_removal(r#"7     rmdir("/tmp/x") = -1 EROFS"#, "`rmdir`")]
    #[case::removal(r#"7     unlinkat(AT_FDCWD, "/tmp/x", 0) = -1 EROFS"#, "`unlinkat`")]
    #[case::hard_link(
        r#"7     linkat(AT_FDCWD, "/a", AT_FDCWD, "/b", 0) = -1 EROFS"#,
        "`linkat`"
    )]
    #[case::symbolic_link(r#"7     symlinkat("/a", AT_FDCWD, "/b") = -1 EROFS"#, "`symlinkat`")]
    #[case::node_creation(
        r#"7     mknodat(AT_FDCWD, "/dev/x", S_IFCHR|0600, makedev(1, 3)) = -1 EROFS"#,
        "`mknodat`"
    )]
    #[case::mode_change(r#"7     fchmodat(AT_FDCWD, "/x", 0600, 0) = -1 EROFS"#, "`fchmodat`")]
    #[case::ownership_change(r#"7     fchownat(AT_FDCWD, "/x", 0, 0, 0) = -1 EROFS"#, "`fchownat`")]
    #[case::extended_open(r#"7     openat2(AT_FDCWD, "/repo/probe", {flags=O_WRONLY|O_CREAT, mode=0666}, 24) = -1 EROFS"#, "write-intent")]
    #[case::network(r#"7     connect(3, {sa_family=AF_INET, sin_port=htons(53), sin_addr=inet_addr("1.1.1.1")}, 16) = -1 ENETUNREACH"#, "network address")]
    #[case::datagram(r#"7     sendto(3, "x", 1, 0, {sa_family=AF_INET6, sin6_port=htons(53), sin6_addr=inet_pton(AF_INET6, "::1")}, 28) = -1 ENETUNREACH"#, "network address")]
    #[case::addressed_message(
        r#"7     sendmsg(3, {msg_name={sa_family=AF_INET, sin_port=htons(53), sin_addr=inet_addr("1.1.1.1")}, msg_namelen=16, msg_iov=[{iov_base="x", iov_len=1}], msg_iovlen=1, msg_controllen=0, msg_flags=0}, 0) = -1 ENETUNREACH"#,
        "network address"
    )]
    fn suppressed_attempts_are_reported(#[case] line: &str, #[case] expected: &str) {
        let findings = attempts(line);

        assert!(
            findings.iter().any(|finding| finding.contains(expected)),
            "the reader should report {expected} in {line:?}, got {findings:?}"
        );
    }

    #[rstest]
    #[case::read_only("7     openat(AT_FDCWD, \"/etc/hosts\", O_RDONLY|O_CLOEXEC) = 3")]
    #[case::extended_read_only("7     openat2(AT_FDCWD, \"/etc/hosts\", {flags=O_RDONLY}, 24) = 3")]
    #[case::null_device("7     openat(AT_FDCWD, \"/dev/null\", O_WRONLY|O_CREAT, 0666) = 3")]
    #[case::local_socket("7     connect(3, {sa_family=AF_UNIX, sun_path=\"/run/bus\"}, 110) = 0")]
    #[case::local_datagram(
        "7     sendto(3, \"x\", 1, 0, {sa_family=AF_UNIX, sun_path=\"/run/bus\"}, 110) = 1"
    )]
    fn read_only_and_local_use_is_accepted(#[case] line: &str) {
        let findings = attempts(line);

        assert!(
            findings.is_empty(),
            "the reader should accept {line:?}, got {findings:?}"
        );
    }

    #[test]
    fn a_clean_trace_reports_nothing() {
        let log = concat!(
            "9     openat(AT_FDCWD, \"/etc/ld.so.cache\", O_RDONLY|O_CLOEXEC) = 3\n",
            "9     openat(AT_FDCWD, \"/repo/KANI_VERSION\", O_RDONLY) = 3\n",
            "9     connect(4, {sa_family=AF_UNIX}, 2) = 0\n",
        );

        assert!(
            attempts(log).is_empty(),
            "a read-only trace must report nothing, got {:?}",
            attempts(log)
        );
    }
}
