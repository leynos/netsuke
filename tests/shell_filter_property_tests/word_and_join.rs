//! OBL-ONE-WORD and OBL-JOIN-SPLIT: a quoted value is one word, and joining
//! inverts splitting.
//!
//! The oracle both obligations share is `shlex::split`, the same lexer the IR
//! uses, which makes each a statement about what a *shell* would read rather
//! than about what the encoder emitted. [`the_split_oracle_sees_the_unquoted_form_as_two_words`]
//! and [`a_naive_join_fails_the_split_round_trip`] are what keep those
//! statements from being satisfied by an encoder that emits one inert word.
use super::property_support::{
    POWERSHELL, SH, encoded_sh, non_empty_word, quote_value, render_with, split, word,
};
use anyhow::{Result, ensure};
use minijinja::value::Value;
use proptest::prelude::*;
use proptest::test_runner::{FileFailurePersistence, TestRunner};
use rstest::rstest;
use std::cell::Cell;

// ---------------------------------------------------------------------------
// OBL-ONE-WORD
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/shell_filter_property_tests.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// A quoted value is exactly one shell word.
    #[test]
    fn sh_quoting_yields_exactly_one_word(value in word()) {
        let encoded = encoded_sh(&value)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        let words = split(&encoded)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        prop_assert_eq!(&words, std::slice::from_ref(&value),
            "quoted {:?} as {:?}", value, encoded);
    }
}

/// The oracle distinguishes quoted from unquoted text.
///
/// `shlex::split` would satisfy the property for any encoder that happened to
/// emit a single inert word, including the identity function on single-word
/// inputs. Showing that it returns *two* words for the unquoted form is what
/// makes the one-word assertion load-bearing.
#[test]
fn the_split_oracle_sees_the_unquoted_form_as_two_words() -> Result<()> {
    let unquoted = split("a b")?;
    ensure!(
        unquoted == ["a", "b"],
        "the oracle should split an unquoted space: {unquoted:?}"
    );
    ensure!(
        split(&encoded_sh("a b")?)?.len() == 1,
        "the oracle should see the quoted form as one word"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// OBL-JOIN-SPLIT
// ---------------------------------------------------------------------------

/// Render `{{ values | shell_join }}` under the environment for `dialect`.
fn join_values(values: &[String], dialect: &str) -> Result<String> {
    let subject = Value::from_serialize(values);
    render_with("{{ values | shell_join }}", dialect, &subject)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/shell_filter_property_tests.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// Joining and splitting are inverse over sequences of words.
    #[test]
    fn shell_join_inverts_word_splitting(values in prop::collection::vec(word(), 0..6)) {
        let joined = join_values(&values, SH)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        let words = split(&joined)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        prop_assert_eq!(words, values, "joined as {:?}", joined);
    }
}

/// The empty list and the list holding one empty word render differently.
///
/// These are the two boundaries a `join` implementation is most likely to
/// conflate: if the empty string were emitted for both, one of the two
/// round-trip directions would silently produce the wrong list. `shlex` reads
/// `""` as no words and `''` as one empty word, so a naive implementation that
/// rendered the second as the first would be caught here and nowhere else.
#[rstest]
fn shell_join_distinguishes_an_empty_list_from_one_empty_word() -> Result<()> {
    let empty_list = join_values(&[], SH)?;
    ensure!(
        empty_list.is_empty(),
        "an empty list renders as the empty string: {empty_list:?}"
    );
    ensure!(
        split(&empty_list)?.is_empty(),
        "the empty string splits into no words"
    );

    let one_empty_word = join_values(&[String::new()], SH)?;
    ensure!(
        one_empty_word == "''",
        "one empty word renders as a quoted pair: {one_empty_word:?}"
    );
    ensure!(
        split(&one_empty_word)? == [String::new()],
        "the quoted pair splits into one empty word"
    );
    Ok(())
}

/// The generated join corpus spans the quoting boundary.
///
/// The elements that matter are the ones a naive `join(" ")` would corrupt: a
/// member containing a space, and a member that is the empty string. Without a
/// tally this property could hold over lists of single inert words and never
/// exercise either.
#[test]
fn the_join_corpus_spans_the_quoting_boundary() {
    let tallies: Cell<(usize, usize, usize)> = Cell::new((0, 0, 0));
    TestRunner::new(ProptestConfig {
        cases: 128,
        ..ProptestConfig::default()
    })
    .run(&prop::collection::vec(word(), 0..6), |values| {
        let (spaced, empty, lists) = tallies.get();
        let has_space = values.iter().any(|value| value.contains(' '));
        let has_empty = values.iter().any(String::is_empty);
        tallies.set((
            spaced + usize::from(has_space),
            empty + usize::from(has_empty),
            lists + 1,
        ));
        Ok(())
    })
    .expect("the join corpus should be generatable");

    let (spaced, empty, lists) = tallies.into_inner();
    assert!(
        spaced > 0 && empty > 0 && lists > 0,
        "the corpus must include spaced and empty elements: \
         {spaced} spaced, {empty} empty, over {lists} lists"
    );
}

/// A naive `join(" ")` fails the property the filter satisfies.
///
/// `xs.join(" ")` looks equivalent for single words and is wrong for anything
/// with a space in it: the shell re-splits that word into two. The control is
/// what separates "the filter joins" from "the filter joins *and quotes*".
#[test]
fn a_naive_join_fails_the_split_round_trip() -> Result<()> {
    let values = vec!["a b".to_owned(), "-C".to_owned()];
    let naive = values.join(" ");
    ensure!(
        naive == "a b -C",
        "the naive join should be the plain space join: {naive:?}"
    );
    ensure!(
        split(&naive)? != values,
        "the naive join should not round-trip, or the control proves nothing"
    );
    ensure!(
        split(&join_values(&values, SH)?)? == values,
        "the filter's own join should round-trip the same input"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// OBL-JOIN-QUOTE-AGREE
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/shell_filter_property_tests.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// `shell_join` is `shell_quote` distributed over a list.
    #[test]
    fn shell_join_agrees_with_quoting_each_element(
        values in prop::collection::vec(non_empty_word(), 1..5),
        power_shell in any::<bool>(),
    ) {
        let dialect = if power_shell { POWERSHELL } else { SH };
        let joined = join_values(&values, dialect)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        let mut individually = Vec::new();
        for value in &values {
            individually.push(quote_value(value, dialect)
                .map_err(|error| TestCaseError::fail(format!("{error:#}")))?);
        }
        prop_assert_eq!(
            &joined,
            &individually.join(" "),
            "shell_join disagreed with shell_quote for dialect {:?}",
            dialect,
        );
    }
}

/// The agreement property is not vacuous: the two elements differ.
///
/// If every element encoded to itself, the joined and individually-quoted
/// forms would agree for any implementation that inserted spaces, and the
/// property would prove nothing. One element that needs quoting is what makes
/// the `join(" ")` in the assertion a real comparison.
#[test]
fn the_agreement_property_has_elements_that_need_quoting() -> Result<()> {
    let values = vec!["a b".to_owned(), "plain".to_owned()];
    let quoted = quote_value("a b", SH)?;
    ensure!(
        quoted != "a b",
        "the witness should require quoting, otherwise this proves nothing"
    );
    let joined = join_values(&values, SH)?;
    ensure!(
        joined == format!("{quoted} plain"),
        "the join should be the quoted witness followed by the plain word: {joined:?}"
    );
    Ok(())
}
