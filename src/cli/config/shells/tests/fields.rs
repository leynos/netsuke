//! Permitted and rejected `executable` and `args` spellings.
//!
//! Host path semantics are exercised for both families on whatever host runs
//! the suite, because the parser takes the family as a parameter rather than
//! reading the build target. The byte budget is tested at its exact boundary,
//! where an off-by-one would otherwise pass unnoticed.

use super::super::{ArgumentsError, ExecutableError, MAX_ARGUMENT_BYTES, MAX_ARGUMENTS};
use super::super::{ShellArguments, ShellExecutable};
use crate::shell_selection::HostFamily;
use rstest::rstest;

/// Assert that `text` is accepted as an executable on `host`.
#[rstest]
#[case::bare_name(HostFamily::UnixLike, "dash")]
#[case::absolute_path(HostFamily::UnixLike, "/usr/local/bin/dash")]
#[case::colon_without_separator(HostFamily::UnixLike, "C:tool")]
#[case::windows_bare_name(HostFamily::Windows, "dash")]
#[case::windows_drive_absolute(HostFamily::Windows, "C:\\tools\\shell.exe")]
#[case::windows_drive_absolute_forward(HostFamily::Windows, "C:/tools/shell.exe")]
#[case::windows_unc_share(HostFamily::Windows, "\\\\server\\share\\shell.exe")]
fn executable_spellings_are_accepted(#[case] host: HostFamily, #[case] text: &str) {
    assert!(
        ShellExecutable::parse(text, host).is_ok(),
        "{text:?} must be accepted on {host:?}"
    );
}

/// Assert that `text` is rejected as an executable on `host`, for `reason`.
#[rstest]
#[case::empty(HostFamily::UnixLike, "", ExecutableError::Empty)]
#[case::nul(HostFamily::UnixLike, "da\0sh", ExecutableError::Nul)]
#[case::dot(HostFamily::UnixLike, ".", ExecutableError::DirectoryReference)]
#[case::dot_dot(HostFamily::UnixLike, "..", ExecutableError::DirectoryReference)]
#[case::relative(HostFamily::UnixLike, "bin/dash", ExecutableError::RelativePath)]
#[case::windows_drive_path_on_unix(
    HostFamily::UnixLike,
    "C:\\tools\\shell.exe",
    ExecutableError::RelativePath
)]
#[case::drive_relative(HostFamily::Windows, "C:tool", ExecutableError::DriveRelative)]
#[case::rooted_without_drive(HostFamily::Windows, "/opt/tool", ExecutableError::RootedWithoutDrive)]
#[case::windows_relative(HostFamily::Windows, "bin\\dash", ExecutableError::RelativePath)]
fn executable_spellings_are_rejected(
    #[case] host: HostFamily,
    #[case] text: &str,
    #[case] expected: ExecutableError,
) {
    assert_eq!(
        ShellExecutable::parse(text, host),
        Err(expected),
        "{text:?} must be rejected on {host:?}"
    );
}

/// Keep the accepted spelling exactly as it was configured.
#[test]
fn executable_preserves_the_configured_spelling() {
    let executable = ShellExecutable::parse("//Tools//Shell.EXE", HostFamily::UnixLike)
        .expect("a rooted path is accepted");
    assert_eq!(executable.as_str(), "//Tools//Shell.EXE");
}

/// Accept a non-empty argument vector, including empty elements.
#[rstest]
#[case::one_element(vec!["-c".to_owned()])]
#[case::empty_elements(vec![String::new(), String::new()])]
#[case::at_the_element_limit(vec!["x".to_owned(); MAX_ARGUMENTS])]
fn argument_vectors_are_accepted(#[case] arguments: Vec<String>) {
    let accepted = ShellArguments::parse(&arguments);
    assert!(
        accepted.is_ok(),
        "{arguments:?} must be accepted: {accepted:?}"
    );
}

/// Reject an empty vector and a vector one element past the limit.
#[rstest]
#[case::empty(Vec::new(), ArgumentsError::Empty)]
#[case::too_many(
    vec!["x".to_owned(); MAX_ARGUMENTS + 1],
    ArgumentsError::TooMany { count: MAX_ARGUMENTS + 1 }
)]
fn argument_counts_are_bounded(#[case] arguments: Vec<String>, #[case] expected: ArgumentsError) {
    assert_eq!(ShellArguments::parse(&arguments), Err(expected));
}

/// Report the first element containing NUL by position.
#[test]
fn a_nul_argument_is_named_by_position() {
    let arguments = vec!["-c".to_owned(), "bad\0arg".to_owned(), "worse\0".to_owned()];
    assert_eq!(
        ShellArguments::parse(&arguments),
        Err(ArgumentsError::Nul { index: 1 })
    );
}

/// Accept a vector exactly at the byte budget and reject one byte past it.
#[test]
fn the_byte_budget_is_exact() {
    let at_budget = vec!["x".repeat(MAX_ARGUMENT_BYTES)];
    assert!(ShellArguments::parse(&at_budget).is_ok());

    let over_budget = vec!["x".repeat(MAX_ARGUMENT_BYTES + 1)];
    assert_eq!(
        ShellArguments::parse(&over_budget),
        Err(ArgumentsError::TooLong)
    );
}

/// Count the elements rather than echoing them.
#[test]
fn arguments_display_only_their_count() {
    let arguments =
        ShellArguments::parse(&["secret-token".to_owned()]).expect("the vector is accepted");
    assert_eq!(arguments.to_string(), "1 argument(s)");
}
