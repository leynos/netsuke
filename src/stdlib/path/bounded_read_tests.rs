//! Tests for the bounded-read boundary, `linecount`, and the incremental
//! UTF-8 validator.
//!
//! `linecount` no longer materializes the file: it counts newline bytes in
//! each fixed-size chunk and carries a multi-byte sequence that a chunk
//! boundary split. A dropped or double-counted chunk, a lost carry, or a
//! binary file counted as text would each only surface here, so the cases
//! below pin the boundary and the carry explicitly.

use anyhow::{Context, Result, anyhow, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::ErrorKind;
use rstest::rstest;
use tempfile::TempDir;

use super::{Utf8Validator, linecount};
use crate::stdlib::path::fs_utils::FileReadLimits;

/// Name of the fixture file staged inside the temporary directory.
const FIXTURE_NAME: &str = "payload";

/// Limits for a fixture that is expected to fit inside the budget.
const UNBOUNDED: FileReadLimits = FileReadLimits {
    max_bytes: u64::MAX,
    follow_symlinks: false,
};

/// Write `payload` to a temporary file and return the directory guard
/// alongside the file's path.
///
/// The write goes through a `cap_std` directory capability rather than
/// ambient `std::fs`, matching the convention the sibling test modules
/// follow. The guard must outlive the returned path: dropping it removes
/// the file.
fn fixture(payload: &[u8]) -> Result<(TempDir, Utf8PathBuf)> {
    let dir = tempfile::tempdir()?;
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
        .map_err(|path| anyhow!("temporary path is not valid UTF-8: {path:?}"))?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())?;
    handle.write(FIXTURE_NAME, payload)?;
    Ok((dir, root.join(FIXTURE_NAME)))
}

/// Limits that permit exactly `budget` bytes.
const fn budget(budget: u64) -> FileReadLimits {
    FileReadLimits {
        max_bytes: budget,
        follow_symlinks: false,
    }
}

/// Strip the Fluent isolate marks that wrap interpolated numbers.
///
/// Interpolation wraps a placeable in first-strong/paragraph-direction
/// isolates, so a message that reads `8 bytes` in the locale file contains
/// invisible characters the assertion has to remove first.
fn readable(message: &str) -> String {
    message.replace(['\u{2068}', '\u{2069}'], "")
}

#[rstest]
#[case::empty("", 0)]
#[case::no_terminator("alpha", 1)]
#[case::single_terminator("alpha\n", 1)]
#[case::blank_line("alpha\n\n", 2)]
#[case::two_terminated_lines("alpha\nbeta\n", 2)]
#[case::trailing_fragment("alpha\nbeta", 2)]
fn linecount_counts_the_reference_lines(#[case] text: &str, #[case] expected: usize) -> Result<()> {
    let (_dir, file) = fixture(text.as_bytes())?;
    let counted = linecount(&file, &UNBOUNDED)?;
    ensure!(
        counted == expected,
        "expected {expected} lines for {text:?} but counted {counted}"
    );
    Ok(())
}

/// A single line larger than the budget must fail without being buffered.
///
/// The previous implementation read until a terminator arrived, so an
/// unbounded line was allocated in full before the budget was consulted.
/// This file has no terminator at all, so only an incremental guard can
/// reject it.
#[test]
fn linecount_rejects_a_line_longer_than_the_budget() -> Result<()> {
    let (_dir, file) = fixture(&[b'x'; 32])?;
    let err = linecount(&file, &budget(8))
        .expect_err("a 32-byte unterminated line must exceed an 8-byte budget");
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "expected InvalidOperation but was {:?}",
        err.kind()
    );
    let message = readable(&err.to_string());
    ensure!(
        message.contains("8 bytes"),
        "the diagnostic should quote the limit: {message}"
    );
    Ok(())
}

#[test]
fn linecount_rejects_undecodable_bytes() -> Result<()> {
    let cases: [(&str, &[u8]); 2] = [
        ("stray continuation", b"alpha\n\xff\xfe\nomega\n"),
        ("truncated final sequence", b"alpha\n\xc3"),
    ];
    for (label, payload) in cases {
        let (_dir, file) = fixture(payload)?;
        let Err(err) = linecount(&file, &UNBOUNDED) else {
            bail!("{label}: invalid UTF-8 must be rejected rather than counted");
        };
        ensure!(
            err.kind() == ErrorKind::InvalidOperation,
            "{label}: expected InvalidOperation but was {:?}",
            err.kind()
        );
    }
    Ok(())
}

/// Valid text is accepted however a chunk boundary splits it.
#[test]
fn validator_accepts_every_split_of_valid_text() -> Result<()> {
    // Two-, three-, and four-byte sequences plus ASCII, so a boundary can
    // fall inside any of them.
    const TEXT: &str = "alpha\n\u{e9}\u{20ac}\u{1f600}\nomega\n";
    for split in 0..=TEXT.len() {
        let (head, tail) = TEXT.as_bytes().split_at(split);
        let mut validator = Utf8Validator::default();
        validator
            .push(head, Utf8Path::new(FIXTURE_NAME))
            .with_context(|| format!("valid text rejected at split {split}"))?;
        validator
            .push(tail, Utf8Path::new(FIXTURE_NAME))
            .with_context(|| format!("valid text rejected at split {split}"))?;
        validator
            .finish(Utf8Path::new(FIXTURE_NAME))
            .with_context(|| format!("valid text left a carry at split {split}"))?;
    }
    Ok(())
}

/// A carry that the next chunk completes with a bad byte is rejected.
#[test]
fn validator_rejects_a_corrupt_carry() -> Result<()> {
    let mut validator = Utf8Validator::default();
    validator.push(b"\xc3", Utf8Path::new(FIXTURE_NAME))?;
    let err = validator
        .push(b"b", Utf8Path::new(FIXTURE_NAME))
        .expect_err("a continuation byte that does not extend the carry must be rejected");
    ensure!(
        err.kind() == ErrorKind::InvalidOperation,
        "expected InvalidOperation but was {:?}",
        err.kind()
    );
    Ok(())
}

/// A file ending inside a multi-byte sequence is not text.
#[test]
fn validator_rejects_a_truncated_final_sequence() -> Result<()> {
    let mut validator = Utf8Validator::default();
    validator.push(b"\xc3", Utf8Path::new(FIXTURE_NAME))?;
    ensure!(
        validator.finish(Utf8Path::new(FIXTURE_NAME)).is_err(),
        "a truncated final sequence must be rejected at the end of the read"
    );
    Ok(())
}

mod properties {
    //! Property tests for the shared bounded-read boundary.
    //!
    //! The cases above pin chosen inputs. The interesting behaviour is the
    //! boundary itself, which the length and budget have to vary together
    //! to reach: a read that consumes exactly the budget must succeed, one
    //! byte more must fail, and no byte may be dropped or duplicated in
    //! between. Generating both sides lets the partition fall at arbitrary
    //! offsets, including inside a multi-byte sequence and either side of
    //! the 8192-byte chunk.

    use anyhow::{Result, anyhow};
    use camino::Utf8PathBuf;
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use proptest::prelude::*;
    use tempfile::TempDir;

    use super::super::{BoundedRead, read_bounded_chunk};
    use super::{FIXTURE_NAME, linecount};
    use crate::stdlib::path::fs_utils::{FileReadLimits, open_file_checked};

    /// Write `payload` to a temporary file and return the directory guard
    /// alongside the file's path. See the parent module's `fixture`.
    fn fixture(payload: &[u8]) -> Result<(TempDir, Utf8PathBuf)> {
        let dir = tempfile::tempdir()?;
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
            .map_err(|path| anyhow!("temporary path is not valid UTF-8: {path:?}"))?;
        let handle = Dir::open_ambient_dir(&root, ambient_authority())?;
        handle.write(FIXTURE_NAME, payload)?;
        Ok((dir, root.join(FIXTURE_NAME)))
    }

    /// Limits permitting exactly `budget` bytes without following links.
    fn limits(budget: u64) -> FileReadLimits {
        FileReadLimits {
            max_bytes: budget,
            follow_symlinks: false,
        }
    }

    proptest! {
        /// A bounded read fails exactly past the budget and loses no byte.
        #[test]
        fn bounded_read_is_length_exact(
            payload in prop::collection::vec(any::<u8>(), 0..20_000),
            budget in 0_usize..20_000,
        ) {
            let budget_u64 = u64::try_from(budget).expect("a usize budget fits u64");
            let (_dir, file) = fixture(&payload).expect("stage the payload");
            let mut handle = open_file_checked(&file, &limits(budget_u64))
                .expect("open the fixture");
            let mut state = BoundedRead::new(budget_u64);
            let mut buffer = [0_u8; 8192];
            let mut collected = Vec::new();
            let mut failure = None;
            loop {
                match read_bounded_chunk(&mut state, &mut handle, &mut buffer, &file) {
                    Ok(Some(chunk)) => collected.extend_from_slice(chunk),
                    Ok(None) => break,
                    Err(err) => {
                        failure = Some(err);
                        break;
                    }
                }
            }
            if payload.len() > budget {
                prop_assert!(
                    failure.is_some(),
                    "a {}-byte read must fail a {budget}-byte budget",
                    payload.len()
                );
            } else {
                prop_assert!(
                    failure.is_none(),
                    "a {}-byte read must fit a {budget}-byte budget: {failure:?}",
                    payload.len()
                );
                prop_assert_eq!(collected, payload);
            }
        }

        /// The count matches a reference split of any text within budget.
        #[test]
        fn linecount_matches_a_reference_split(
            chars in prop::collection::vec(
                prop::sample::select(vec!['a', '\n', '\u{e9}', '\u{20ac}', '\u{1f600}']),
                0..4_000,
            ),
            budget in 0_usize..20_000,
        ) {
            let text: String = chars.into_iter().collect();
            let (_dir, file) = fixture(text.as_bytes()).expect("stage the payload");
            let counted = linecount(
                &file,
                &limits(u64::try_from(budget).expect("a usize budget fits u64")),
            );
            if text.len() > budget {
                prop_assert!(
                    counted.is_err(),
                    "a {}-byte file must fail a {budget}-byte budget, but counted {counted:?}",
                    text.len()
                );
            } else {
                let total = counted.expect("a file within budget must be counted");
                prop_assert_eq!(total, text.lines().count());
            }
        }
    }
}
