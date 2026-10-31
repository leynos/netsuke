//! Unit tests for the `shell_join` intermediate-byte ceiling.
//!
//! The behavioural suites drive the same ceiling through a real `netsuke`
//! process. These cases pin the boundary arithmetic where a failure names the
//! helper rather than a manifest: the pre-check, the separator's contribution,
//! the exact PowerShell length, and the post-encode check that `Sh` needs
//! because its output can be three times its input.

use minijinja::value::Value;
use rstest::rstest;
use test_support::{EnLocalizer, en_localizer};

use super::{SHELL_JOIN_MAX_ENCODED_BYTES, join_sequence};
use crate::shell_word::ShellDialect;

/// Join `values` under `dialect`, as the filter would.
fn join(values: &[String], dialect: ShellDialect) -> Result<String, minijinja::Error> {
    join_sequence(dialect, &Value::from_serialize(values))
}

/// Half the ceiling, written as a shift.
///
/// The workspace denies `clippy::integer_division` and
/// `clippy::integer_division_remainder_used`, so a literal `/` here would fail
/// the build. The ceiling is a power of two, so the shift is exact.
const HALF_CEILING: usize = SHELL_JOIN_MAX_ENCODED_BYTES >> 1;

/// One inert word of `length` bytes.
///
/// `'a'` is inert to the `Sh` encoder, so the word's input and encoded lengths
/// coincide; that keeps these cases about the ceiling rather than about
/// escaping.
fn inert_word(length: usize) -> String {
    "a".repeat(length)
}

/// Return `value` as Fluent renders an interpolated argument.
///
/// The formatter wraps every placeable in U+2068 (first-strong isolate) and
/// U+2069 (pop directional isolate) so a translated sentence keeps the
/// argument's direction. The wrappers are invisible in a terminal, so an
/// assertion written as plain text would never match even though the reader
/// sees `item 0`. Composing them here states that contract rather than
/// stripping the wrappers away.
fn isolated(value: impl std::fmt::Display) -> String {
    format!("\u{2068}{value}\u{2069}")
}

/// The ceiling is one mebibyte, not a round decimal megabyte.
///
/// Pinned because the localized diagnostic prints it: a change here is a
/// user-visible change and should be a deliberate one.
#[test]
fn the_ceiling_is_one_mebibyte() {
    assert_eq!(
        SHELL_JOIN_MAX_ENCODED_BYTES,
        1024 * 1024,
        "the shell_join ceiling is a fixed helper-local constant"
    );
}

/// A word that encodes to exactly the ceiling is still accepted.
///
/// The boundary is inclusive: the check rejects only a result that would be
/// larger. For `sh`, an inert word encodes to itself, so input length and
/// encoded length coincide.
#[test]
fn a_word_of_exactly_the_ceiling_is_accepted() {
    let word = inert_word(SHELL_JOIN_MAX_ENCODED_BYTES);
    let joined = join(&[word], ShellDialect::Sh).expect("a word at the ceiling should encode");
    assert_eq!(
        joined.len(),
        SHELL_JOIN_MAX_ENCODED_BYTES,
        "the accepted boundary value should fill the ceiling exactly"
    );
}

/// One byte past the ceiling is rejected before the word is encoded.
#[rstest]
fn a_word_past_the_ceiling_is_rejected(en_localizer: EnLocalizer) {
    let _localizer = en_localizer;
    let word = inert_word(SHELL_JOIN_MAX_ENCODED_BYTES + 1);
    let error =
        join(&[word], ShellDialect::Sh).expect_err("a word past the ceiling must not be encoded");
    let message = error.to_string();
    assert!(
        message.contains("[netsuke::jinja::shell::join_limit]"),
        "the rejection should carry the stable diagnostic code: {message}"
    );
    assert!(
        message.contains(&SHELL_JOIN_MAX_ENCODED_BYTES.to_string()),
        "the rejection should name the ceiling: {message}"
    );
    assert!(
        message.contains(&format!("item {}", isolated(0))),
        "the rejection should name the offending index: {message}"
    );
    assert!(
        !message.contains("aaaa"),
        "the rejection must not echo the content it refused: {message}"
    );
}

/// The separator counts against the ceiling, so the sum of members is not the
/// whole story.
///
/// Two members that each fit on their own cannot be joined once the space
/// between them pushes the total past the ceiling. The reported index is the
/// member that did not fit, not the member that crossed on its own.
#[rstest]
fn the_separator_counts_towards_the_ceiling(en_localizer: EnLocalizer) {
    let _localizer = en_localizer;
    let values = vec![inert_word(HALF_CEILING), inert_word(HALF_CEILING)];
    let error = join(&values, ShellDialect::Sh)
        .expect_err("the joining space must count against the limit");
    let message = error.to_string();
    assert!(
        message.contains(&format!("item {}", isolated(1))),
        "the member that could not fit is the second one: {message}"
    );
}

/// The reported trigger: three references to one generated string.
///
/// A compact manifest can repeat a reference rather than repeat the text, so
/// the rendered sequence is much larger than the manifest. This is the shape
/// issue #970 reported; it must fail at the third member rather than allocating
/// the joined text first.
#[rstest]
fn repeated_references_stop_at_the_member_that_cannot_fit(en_localizer: EnLocalizer) {
    let _localizer = en_localizer;
    let word = inert_word(400_000);
    let values = vec![word.clone(), word.clone(), word];
    let error =
        join(&values, ShellDialect::Sh).expect_err("three 400 kB members exceed the ceiling");
    let message = error.to_string();
    assert!(
        message.contains(&format!("item {}", isolated(2))),
        "the third member is the first that cannot fit: {message}"
    );
}

/// A word that doubles when encoded is rejected by both dialects, each through
/// a different check.
///
/// Every quote becomes `\'` in `Sh` and a doubled quote in PowerShell, so the
/// input is half a ceiling of quotes in both cases. PowerShell's length is
/// computed exactly, so the pre-check rejects the word before it is encoded; a
/// check that used the input length would let it through and only discover the
/// overshoot afterwards. `Sh`'s pre-check is a lower bound that this input
/// passes, so only the post-encode check can reject it — which is what proves
/// that second check exists.
#[rstest]
#[case::power_shell_pre_check(ShellDialect::PowerShell)]
#[case::sh_post_encode_check(ShellDialect::Sh)]
fn a_word_that_doubles_past_the_ceiling_is_rejected(
    #[case] dialect: ShellDialect,
    en_localizer: EnLocalizer,
) {
    let _localizer = en_localizer;
    let quotes = "'".repeat(HALF_CEILING + 1);
    let error = join(&[quotes], dialect)
        .expect_err("a word that doubles when encoded must not pass the ceiling");
    assert!(
        error.to_string().contains(&format!("item {}", isolated(0))),
        "the single member should be named: {error}"
    );
}

/// A doubled word that lands exactly on the ceiling is accepted, in both
/// dialects.
///
/// The controls for [`a_word_that_doubles_past_the_ceiling_is_rejected`]: the
/// same input shapes one quote shorter fit exactly, so the rejections above are
/// about the ceiling and not about quotes being refused. `Sh` fills the ceiling
/// from half a ceiling of quotes. PowerShell spends two bytes on its
/// delimiters, so it needs one quote fewer.
#[rstest]
#[case::sh_post_encode_check(ShellDialect::Sh, HALF_CEILING)]
#[case::power_shell_inclusive(ShellDialect::PowerShell, HALF_CEILING - 1)]
fn a_word_that_fills_the_ceiling_exactly_is_accepted(
    #[case] dialect: ShellDialect,
    #[case] quotes: usize,
) {
    let joined = join(&["'".repeat(quotes)], dialect).expect("a word that just fits should encode");
    assert_eq!(
        joined.len(),
        SHELL_JOIN_MAX_ENCODED_BYTES,
        "{dialect:?} should fill the ceiling exactly"
    );
}
