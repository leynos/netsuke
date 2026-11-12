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
#[case::escaped_quote_in_path(
    r#"7     openat(AT_FDCWD, "/tmp/a\"b", O_WRONLY|O_CREAT) = -1 EROFS (Read-only file system)"#,
    "write-intent"
)]
#[case::device_named_inside_an_escaped_quote(
    r#"7     openat(AT_FDCWD, "/tmp/\"/dev/null", O_WRONLY|O_CREAT) = -1 EROFS"#,
    "write-intent"
)]
#[case::directory_creation(r#"7     mkdir("/tmp/x", 0777) = -1 EROFS"#, "`mkdir`")]
#[case::directory_creation_at(r#"7     mkdirat(AT_FDCWD, "/tmp/x", 0777) = -1 EROFS"#, "`mkdirat`")]
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
#[case::extended_open(
    r#"7     openat2(AT_FDCWD, "/repo/probe", {flags=O_WRONLY|O_CREAT, mode=0666}, 24) = -1 EROFS"#,
    "write-intent"
)]
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
