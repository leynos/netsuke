//! Verify PowerShell token recognition uses source-relative character positions.

use super::PowerShellSourceCharacter;
use rstest::rstest;

struct TokenScenario {
    source: &'static str,
    position: usize,
    expected: bool,
}

#[rstest]
#[case::after_space(TokenScenario { source: " #", position: 1, expected: true })]
#[case::after_nonbreaking_space(TokenScenario { source: "\u{00a0}#", position: 1, expected: true })]
#[case::after_ideographic_space(TokenScenario { source: "\u{3000}#", position: 1, expected: true })]
#[case::after_line_separator(TokenScenario { source: "\u{2028}#", position: 1, expected: true })]
#[case::after_next_line_control(TokenScenario { source: "\u{0085}#", position: 1, expected: false })]
#[case::after_assignment(TokenScenario { source: "=#", position: 1, expected: false })]
#[case::after_closing_bracket(TokenScenario { source: "]#", position: 1, expected: false })]
#[case::after_asterisk(TokenScenario { source: "*#", position: 1, expected: false })]
#[case::after_plus(TokenScenario { source: "+#", position: 1, expected: false })]
#[case::after_redirection(TokenScenario { source: ">#", position: 1, expected: false })]
fn classify_comment_boundaries(#[case] scenario: TokenScenario) {
    let characters = scenario.source.chars().collect::<Vec<_>>();
    let position = PowerShellSourceCharacter::new(&characters, scenario.position)
        .expect("the scenario position identifies a source character");

    assert_eq!(position.is_token_boundary(), scenario.expected);
    let first =
        PowerShellSourceCharacter::new(&['#'], 0).expect("the first source character exists");
    assert!(first.is_token_boundary());
}

#[rstest]
#[case::after_assignment(TokenScenario { source: "=@'", position: 1, expected: true })]
#[case::after_closing_bracket(TokenScenario { source: "]@'", position: 1, expected: true })]
#[case::after_space(TokenScenario { source: " @'", position: 1, expected: true })]
#[case::after_asterisk(TokenScenario { source: "*@'", position: 1, expected: false })]
fn classify_here_string_boundaries(#[case] scenario: TokenScenario) {
    let characters = scenario.source.chars().collect::<Vec<_>>();
    let position = PowerShellSourceCharacter::new(&characters, scenario.position)
        .expect("the scenario position identifies a source character");

    assert_eq!(position.is_here_string_boundary(), scenario.expected);
}

#[rstest]
#[case::after_postfix_increment(TokenScenario { source: "$i++#", position: 4, expected: true })]
#[case::after_postfix_decrement(TokenScenario { source: "$i--#", position: 4, expected: true })]
#[case::after_member_postfix_increment(TokenScenario { source: "$obj.Value++#", position: 12, expected: true })]
#[case::after_member_postfix_decrement(TokenScenario { source: "$obj.Value--#", position: 12, expected: true })]
#[case::after_indexed_postfix_increment(TokenScenario { source: "$arr[0]++#", position: 9, expected: true })]
#[case::after_indexed_postfix_decrement(TokenScenario { source: "$obj.Items[0]--#", position: 15, expected: true })]
#[case::after_assignment(TokenScenario { source: "a=#", position: 2, expected: true })]
#[case::after_bracket(TokenScenario { source: "a]#", position: 2, expected: true })]
#[case::compact_addition_expression(TokenScenario { source: "$x=1+#", position: 5, expected: true })]
#[case::compact_multiplication_expression(TokenScenario { source: "$x=2*#", position: 5, expected: true })]
#[case::compact_member_expression(TokenScenario { source: "$n=$s.Length#", position: 12, expected: true })]
#[case::indexed_member_expression(TokenScenario { source: "$n=$s[0].Length#", position: 15, expected: true })]
#[case::decimal_literal(TokenScenario { source: "$x=5#", position: 4, expected: true })]
#[case::hexadecimal_literal(TokenScenario { source: "$x=0x10#", position: 7, expected: true })]
#[case::size_suffixed_literal(TokenScenario { source: "$x=1kb#", position: 6, expected: true })]
#[case::spaced_numeric_literal(TokenScenario { source: "Write-Output 5#", position: 14, expected: true })]
#[case::redirection_operator(TokenScenario { source: "Write-Output value >#", position: 20, expected: true })]
#[case::numeric_argument_suffix(TokenScenario { source: "foo5#", position: 4, expected: false })]
#[case::numeric_looking_argument(TokenScenario { source: "foo*5#", position: 5, expected: false })]
#[case::after_argument_asterisk(TokenScenario { source: "foo*#", position: 4, expected: false })]
#[case::after_argument_hash(TokenScenario { source: "foo#", position: 3, expected: false })]
#[case::at_token_boundary(TokenScenario { source: "foo #", position: 4, expected: false })]
#[case::at_start_of_source(TokenScenario { source: "#", position: 0, expected: false })]
fn classify_ambiguous_expression_hashes(#[case] scenario: TokenScenario) {
    let characters = scenario.source.chars().collect::<Vec<_>>();
    let position = PowerShellSourceCharacter::new(&characters, scenario.position)
        .expect("the scenario position identifies a source character");

    assert_eq!(
        position.hash_follows_ambiguous_expression_boundary(),
        scenario.expected
    );
}

#[test]
fn recognize_stop_parsing_token_after_unicode_whitespace() {
    let characters = "\u{00a0}--%\u{3000}".chars().collect::<Vec<_>>();
    let position = PowerShellSourceCharacter::new(&characters, 1)
        .expect("the token begins at the expected character index");

    assert!(position.starts_stop_parsing_token());
}

#[test]
fn source_positions_keep_unicode_character_indices_and_source_edges() {
    let characters = "é --%".chars().collect::<Vec<_>>();
    let delimiter = PowerShellSourceCharacter::new(&characters, 1)
        .expect("space follows the non-ASCII character");
    let token = delimiter.next().expect("the token follows the space");
    let final_character =
        PowerShellSourceCharacter::new(&characters, 4).expect("the source has a final character");

    assert_eq!(token.value(), '-');
    assert_eq!(token.position(), 2);
    assert!(token.starts_stop_parsing_token());
    assert!(PowerShellSourceCharacter::new(&characters, 5).is_none());
    assert!(
        PowerShellSourceCharacter::new(&characters, 0)
            .expect("the source starts with a character")
            .previous()
            .is_none()
    );
    assert!(final_character.next().is_none());
}
