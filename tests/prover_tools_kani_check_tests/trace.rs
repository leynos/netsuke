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
/// text, since nothing after it can be read as an argument.
fn unquoted(text: &str) -> String {
    let mut kept = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((before, after)) = rest.split_once('"') {
        kept.push_str(before);
        rest = after.split_once('"').map_or("", |(_, tail)| tail);
    }
    kept.push_str(rest);
    kept
}

/// Report whether the path is one of the character devices a caller writes to.
///
/// The path is compared whole, closing quote included. `/dev/null` is a device
/// whose write every program makes; `/dev/null.log` is an ordinary file, and a
/// substring test would exempt a write to it as if it were the device.
fn is_device(arguments: &str) -> bool {
    DEVICE_PATHS
        .iter()
        .any(|device| arguments.contains(&format!("\"{device}\"")))
}

#[cfg(test)]
mod tests {
    //! Cases for the reader, so the oracle is known to fire rather than known
    //! to pass on today's log.

    use super::{DEVICE_PATHS, WRITE_FLAGS, attempts};
    use proptest::prelude::*;
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
    #[case::device_named_file(
        r#"7     openat(AT_FDCWD, "/dev/null.log", O_WRONLY|O_CREAT|O_TRUNC, 0666) = -1 EROFS (Read-only file system)"#,
        "write-intent"
    )]
    #[case::device_path_as_a_prefix(
        r#"7     openat(AT_FDCWD, "/dev/ttyUSB0", O_WRONLY) = -1 EROFS (Read-only file system)"#,
        "write-intent"
    )]
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
    #[case::legacy_time_change(
        r#"7     utimes("/x", [{tv_sec=1, tv_usec=0}, {tv_sec=1, tv_usec=0}]) = -1 EROFS"#,
        "`utimes`"
    )]
    #[case::time_change(r#"7     utime("/x", [1234567890, 1234567890]) = -1 EROFS"#, "`utime`")]
    #[case::at_time_change(
        r#"7     futimesat(AT_FDCWD, "/x", [{tv_sec=1, tv_usec=0}, {tv_sec=1, tv_usec=0}]) = -1 EROFS"#,
        "`futimesat`"
    )]
    #[case::ownership_change(r#"7     fchownat(AT_FDCWD, "/x", 0, 0, 0) = -1 EROFS"#, "`fchownat`")]
    #[case::link_ownership_change(r#"7     lchown("/x", 0, 0) = -1 EROFS"#, "`lchown`")]
    #[case::attribute_write(
        r#"7     setxattr("/x", "user.tag", "v", 1, 0) = -1 EROFS"#,
        "`setxattr`"
    )]
    #[case::attribute_removal(r#"7     removexattr("/x", "user.tag") = -1 EROFS"#, "`removexattr`")]
    #[case::extended_open(r#"7     openat2(AT_FDCWD, "/repo/probe", {flags=O_WRONLY|O_CREAT, mode=0666}, 24) = -1 EROFS"#, "write-intent")]
    #[case::network(r#"7     connect(3, {sa_family=AF_INET, sin_port=htons(53), sin_addr=inet_addr("1.1.1.1")}, 16) = -1 ENETUNREACH"#, "network address")]
    #[case::datagram(r#"7     sendto(3, "x", 1, 0, {sa_family=AF_INET6, sin6_port=htons(53), sin6_addr=inet_pton(AF_INET6, "::1")}, 28) = -1 ENETUNREACH"#, "network address")]
    #[case::addressed_message(
        r#"7     sendmsg(3, {msg_name={sa_family=AF_INET, sin_port=htons(53), sin_addr=inet_addr("1.1.1.1")}, msg_namelen=16, msg_iov=[{iov_base="x", iov_len=1}], msg_iovlen=1, msg_controllen=0, msg_flags=0}, 0) = -1 ENETUNREACH"#,
        "network address"
    )]
    #[case::addressed_messages(
        r#"7     sendmmsg(3, [{msg_hdr={msg_name={sa_family=AF_INET, sin_port=htons(53), sin_addr=inet_addr("1.1.1.1")}, msg_namelen=16, msg_iov=[{iov_base="x", iov_len=1}], msg_iovlen=1, msg_controllen=0, msg_flags=0}, msg_len=1}], 1, 0) = -1 ENETUNREACH"#,
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
    #[case::path_naming_a_write_flag(
        "7     openat(AT_FDCWD, \"/repo/O_CREAT.log\", O_RDONLY|O_CLOEXEC) = 3"
    )]
    #[case::local_socket("7     connect(3, {sa_family=AF_UNIX, sun_path=\"/run/bus\"}, 110) = 0")]
    #[case::local_socket_naming_a_remote_family(
        "7     connect(3, {sa_family=AF_UNIX, sun_path=\"/run/AF_INET\"}, 110) = 0"
    )]
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

    /// The path fragments a generated case assembles its path from.
    ///
    /// The set deliberately names open flags, an address family, and the
    /// components of a character device. A path built from those is what
    /// separates a reader that reads the flags from one that searches the whole
    /// line for them.
    const PATH_FRAGMENTS: [&str; 8] = [
        "repo", "tmp", "O_CREAT", "O_TRUNC", "AF_INET", "dev", "null", "log",
    ];

    /// Build a path of one to three fragments drawn from [`PATH_FRAGMENTS`].
    fn generated_path() -> impl Strategy<Value = String> {
        prop::collection::vec(prop::sample::select(&PATH_FRAGMENTS), 1..=3)
            .prop_map(|fragments| format!("/{}", fragments.join("/")))
    }

    proptest! {
        /// Check that a write finding follows the open flags, not the path.
        ///
        /// The only path text that can change the answer is a whole character
        /// device, which is exempt because every program writes to one.
        #[test]
        fn classification_follows_the_open_flags(
            path in generated_path(),
            write_flags in prop::collection::vec(prop::sample::select(&WRITE_FLAGS), 0..=3),
        ) {
            let flags = if write_flags.is_empty() {
                "O_RDONLY".to_owned()
            } else {
                write_flags.join("|")
            };
            let line = format!("7     openat(AT_FDCWD, \"{path}\", {flags}) = -1 EROFS");
            let expected = !write_flags.is_empty() && !DEVICE_PATHS.contains(&path.as_str());

            let findings = attempts(&line);
            prop_assert_eq!(
                !findings.is_empty(),
                expected,
                "the open of {:?} with {:?} should report a write: {}",
                path,
                flags,
                expected
            );
        }

        /// Check that a network finding follows the address family, not a path.
        ///
        /// A Unix socket is local however its path is named, and a remote
        /// address is remote however the path alongside it is named.
        #[test]
        fn only_an_address_selector_outside_a_path_names_a_remote_host(
            path in generated_path(),
            family in prop::sample::select(&["AF_UNIX", "AF_INET", "AF_INET6"]),
        ) {
            let address = if family == "AF_UNIX" {
                format!("{{sa_family=AF_UNIX, sun_path=\"{path}\"}}")
            } else {
                format!("{{sa_family={family}, sin_addr=inet_addr(\"1.1.1.1\")}}")
            };
            let line = format!("7     connect(3, {address}, 16) = -1 ENETUNREACH");

            let findings = attempts(&line);
            prop_assert_eq!(
                !findings.is_empty(),
                family != "AF_UNIX",
                "the connect to {} should be classified by its family: {}",
                address,
                line
            );
        }
    }
}
