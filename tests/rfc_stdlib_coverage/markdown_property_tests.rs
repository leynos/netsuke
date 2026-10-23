//! Compare fence state transitions with independent boundary predicates.
use super::{Fences, is_separator, row_cells};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use rstest::rstest;

/// Render one generated delimiter candidate.
fn candidate(character: char, run: usize, indentation: (usize, bool), info: &str) -> String {
    let (indent, tab) = indentation;
    let prefix = if tab {
        "\t".to_owned()
    } else {
        " ".repeat(indent)
    };
    format!("{prefix}{}{info}", character.to_string().repeat(run))
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/rfc_stdlib_coverage/markdown_property_tests.proptest-regressions"))),
        .. ProptestConfig::default()
    })]
    #[test]
    fn opening_boundaries_preserve_later_structure(
        tilde in any::<bool>(), run in 0usize..9, indent in 0usize..7,
        tab in any::<bool>(), info in prop::sample::select(vec!["", " rust", "  ", " x`y", " ~"])) {
        let character = if tilde { '~' } else { '`' };
        let line = candidate(character, run, (indent, tab), info);
        // The model states grammar constraints directly, without inspecting parsed delimiters.
        let legal = !tab && indent <= 3 && run >= 3 && (tilde || !info.contains('`'));
        let mut fences = Fences::default();
        prop_assert_eq!(fences.mark(&line), legal);
        prop_assert_eq!(fences.mark("### Heading"), legal);
        prop_assert_eq!(fences.mark("| Header |"), legal);
    }

    #[test]
    fn closing_requires_matching_delimiter_sufficient_length_and_no_info(
        opening_tilde in any::<bool>(), closing_tilde in any::<bool>(),
        opening_run in 3usize..9, closing_run in 0usize..11,
        indent in 0usize..7, tab in any::<bool>(),
        info in prop::sample::select(vec!["", "  ", " rust", " x`y", " ~"])) {
        let opening_character = if opening_tilde { '~' } else { '`' };
        let closing_character = if closing_tilde { '~' } else { '`' };
        let mut fences = Fences::default();
        prop_assert!(fences.mark(&candidate(opening_character, opening_run, (0, false), " rust")));
        let closes = !tab && indent <= 3 && opening_tilde == closing_tilde
            && closing_run >= opening_run && info.trim().is_empty();
        prop_assert!(fences.mark(&candidate(closing_character, closing_run, (indent, tab), info)));
        prop_assert_eq!(fences.mark("### After"), !closes);
        prop_assert_eq!(fences.mark("| a | b |"), !closes);
    }
}

#[rstest]
#[case::empty("")]
#[case::single_pipe("|")]
#[case::no_left("a |")]
#[case::no_right("| a")]
fn malformed_rows_are_not_tables(#[case] line: &str) {
    assert!(row_cells(line).is_none());
}

#[rstest]
#[case::aligned(vec![":---:".into(), " --- ".into()], true)]
#[case::empty_row(vec![], false)]
#[case::empty_cell(vec![String::new()], false)]
#[case::ordinary_cell(vec!["data".into()], false)]
fn separator_vocabulary_is_narrow(#[case] cells: Vec<String>, #[case] expected: bool) {
    assert_eq!(is_separator(&cells), expected);
}
