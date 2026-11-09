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

/// The syscalls whose appearance can only mean an attempt to change the host.
///
/// The open family is included because a write-intent open is an attempt even
/// when it is refused; the remaining calls create, remove, rename or re-time a
/// path. `connect` is the one network call a version query could plausibly
/// reach for.
pub(crate) const TRACE_SYSCALLS: &str = concat!(
    "open,openat,creat,mkdir,mkdirat,unlink,unlinkat,rename,renameat,",
    "renameat2,truncate,ftruncate,chmod,fchmodat,utimensat,connect",
);

/// Open flags that ask for the path to be written rather than read.
const WRITE_FLAGS: [&str; 5] = ["O_WRONLY", "O_RDWR", "O_CREAT", "O_TRUNC", "O_APPEND"];

/// Syscalls that create, remove, rename, re-time or resize a path outright.
///
/// `creat` is here rather than with the open family because it takes no flag
/// argument: creating the file is the whole of what it does.
const MUTATING_SYSCALLS: [&str; 12] = [
    "creat",
    "mkdir",
    "mkdirat",
    "unlink",
    "unlinkat",
    "rename",
    "renameat",
    "renameat2",
    "utimensat",
    "truncate",
    "ftruncate",
    "chmod",
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
fn finding(line: &str) -> Option<String> {
    let (prefix, arguments) = line.split_once('(')?;
    let name = prefix.split_whitespace().last()?;
    if MUTATING_SYSCALLS.contains(&name) || name == "fchmodat" {
        return Some(format!("`{name}` changes a path"));
    }
    if name == "connect" {
        return (arguments.contains("AF_INET") || arguments.contains("AF_INET6"))
            .then(|| format!("`connect` to a network address: {arguments}"));
    }
    if !(name == "open" || name == "openat") {
        return None;
    }
    if !WRITE_FLAGS.iter().any(|flag| arguments.contains(flag)) {
        return None;
    }
    if DEVICE_PATHS.iter().any(|device| arguments.contains(device)) {
        return None;
    }
    Some(format!("a write-intent `{name}`: {arguments}"))
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
    #[case::removal(r#"7     unlinkat(AT_FDCWD, "/tmp/x", 0) = -1 EROFS"#, "`unlinkat`")]
    #[case::network(r#"7     connect(3, {sa_family=AF_INET, sin_port=htons(53), sin_addr=inet_addr("1.1.1.1")}, 16) = -1 ENETUNREACH"#, "network address")]
    fn suppressed_attempts_are_reported(#[case] line: &str, #[case] expected: &str) {
        let findings = attempts(line);

        assert!(
            findings.iter().any(|finding| finding.contains(expected)),
            "the reader should report {expected} in {line:?}, got {findings:?}"
        );
    }

    #[rstest]
    #[case::read_only("7     openat(AT_FDCWD, \"/etc/hosts\", O_RDONLY|O_CLOEXEC) = 3")]
    #[case::null_device("7     openat(AT_FDCWD, \"/dev/null\", O_WRONLY|O_CREAT, 0666) = 3")]
    #[case::local_socket("7     connect(3, {sa_family=AF_UNIX, sun_path=\"/run/bus\"}, 110) = 0")]
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
