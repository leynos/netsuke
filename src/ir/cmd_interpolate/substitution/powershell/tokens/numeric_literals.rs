//! Recognize numeric literals that can precede ambiguous PowerShell comments.

use super::PowerShellSourceCharacter;

/// Pair a PowerShell integer radix prefix with its valid digit base.
#[derive(Clone, Copy)]
enum PowerShellIntegerRadix {
    /// Select the `0b` prefix and binary digits.
    Binary,
    /// Select the `0x` prefix and hexadecimal digits.
    Hexadecimal,
}

impl PowerShellIntegerRadix {
    /// Return the prefix spelling used for this radix.
    const fn prefix(self) -> &'static str {
        match self {
            Self::Binary => "0b",
            Self::Hexadecimal => "0x",
        }
    }

    /// Return the digit base used for this radix.
    const fn base(self) -> u32 {
        match self {
            Self::Binary => 2,
            Self::Hexadecimal => 16,
        }
    }
}

/// Report whether a hash follows a numeric literal at an expression boundary.
pub(super) fn hash_follows_numeric_literal(hash: PowerShellSourceCharacter<'_>) -> bool {
    let Some(mut literal_start) = hash.previous() else {
        return false;
    };
    while let Some(previous) = literal_start
        .previous()
        .filter(|character| is_numeric_literal_character(*character))
    {
        literal_start = previous;
    }
    let Some(literal_characters) = hash.characters.get(literal_start.position..hash.position)
    else {
        return false;
    };
    let literal = literal_characters.iter().collect::<String>();
    is_power_shell_numeric_literal(&literal) && has_numeric_expression_boundary(literal_start)
}

/// Report whether a binary operator follows a numeric expression operand.
pub(super) fn operator_follows_numeric_literal(operator: PowerShellSourceCharacter<'_>) -> bool {
    let Some(mut literal_end) = operator.previous() else {
        return false;
    };
    while literal_end.is_power_shell_whitespace() {
        let Some(previous) = literal_end.previous() else {
            return false;
        };
        literal_end = previous;
    }
    let mut literal_start = literal_end;
    while let Some(previous) = literal_start
        .previous()
        .filter(|character| is_numeric_literal_character(*character))
    {
        literal_start = previous;
    }
    let Some(literal_characters) = operator
        .characters
        .get(literal_start.position..literal_end.position.saturating_add(1))
    else {
        return false;
    };
    let literal = literal_characters.iter().collect::<String>();
    is_power_shell_numeric_literal(&literal)
}

/// Recognize a number at statement boundaries, assignments or unary signs.
fn has_numeric_expression_boundary(literal_start: PowerShellSourceCharacter<'_>) -> bool {
    let Some(previous) = literal_start.previous() else {
        return true;
    };
    if previous.is_token_separator() || previous.value() == '=' {
        return true;
    }
    if !matches!(previous.value(), '+' | '-') {
        return false;
    }
    previous.previous().is_none_or(|before_sign| {
        before_sign.is_token_separator() || matches!(before_sign.value(), '=' | '(')
    })
}

/// Include exponent signs only when they belong to the numeric token.
fn is_numeric_literal_character(character: PowerShellSourceCharacter<'_>) -> bool {
    character.value().is_ascii_alphanumeric()
        || matches!(character.value(), '_' | '.')
        || is_exponent_sign(character)
}

/// Recognize a sign immediately after an exponent marker and its coefficient.
fn is_exponent_sign(character: PowerShellSourceCharacter<'_>) -> bool {
    if !matches!(character.value(), '+' | '-') {
        return false;
    }
    let Some(exponent) = character.previous() else {
        return false;
    };
    if !matches!(exponent.value(), 'e' | 'E') {
        return false;
    }
    exponent.previous().is_some_and(|coefficient| {
        coefficient.value().is_ascii_digit() || coefficient.value() == '_'
    })
}

/// Check PowerShell decimal, radix-prefixed and suffixed numeric spellings.
fn is_power_shell_numeric_literal(literal: &str) -> bool {
    let normalized = literal.replace('_', "");
    let unsigned = normalized.trim_start_matches(['+', '-']);
    let without_suffix = strip_numeric_suffix(unsigned);
    is_radix_integer(unsigned, PowerShellIntegerRadix::Hexadecimal)
        || is_radix_integer(unsigned, PowerShellIntegerRadix::Binary)
        || is_radix_integer(without_suffix, PowerShellIntegerRadix::Hexadecimal)
        || is_radix_integer(without_suffix, PowerShellIntegerRadix::Binary)
        || is_decimal(without_suffix)
}

/// Remove all type and multiplier suffixes supported by PowerShell literals.
fn strip_numeric_suffix(mut literal: &str) -> &str {
    while let Some(without_suffix) = strip_one_numeric_suffix(literal) {
        literal = without_suffix;
    }
    literal
}

/// Remove one type or multiplier suffix supported by PowerShell literals.
fn strip_one_numeric_suffix(literal: &str) -> Option<&str> {
    ["kb", "mb", "gb", "tb", "pb", "ul", "lu", "u", "l", "d", "f"]
        .into_iter()
        .find_map(|suffix| {
            literal
                .get(..literal.len().checked_sub(suffix.len())?)
                .filter(|number| {
                    literal
                        .get(number.len()..)
                        .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
                })
        })
}

/// Validate a radix-prefixed integer with at least one digit.
fn is_radix_integer(literal: &str, radix: PowerShellIntegerRadix) -> bool {
    let prefix = radix.prefix();
    literal
        .get(..prefix.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
        && literal.get(prefix.len()..).is_some_and(|digits| {
            !digits.is_empty() && digits.chars().all(|digit| digit.is_digit(radix.base()))
        })
}

/// Validate a decimal integer, fraction or exponent without converting it.
fn is_decimal(literal: &str) -> bool {
    let mut characters = literal.chars().peekable();
    let mut has_digit = consume_digits(&mut characters);
    if characters.next_if_eq(&'.').is_some() {
        has_digit |= consume_digits(&mut characters);
    }
    if !has_digit {
        return false;
    }
    if matches!(characters.peek(), Some('e' | 'E')) {
        characters.next();
        if matches!(characters.peek(), Some('+' | '-')) {
            characters.next();
        }
        if !consume_digits(&mut characters) {
            return false;
        }
    }
    characters.next().is_none()
}

/// Consume an optionally empty run of decimal digits.
fn consume_digits(characters: &mut std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    let mut has_digit = false;
    while characters.peek().is_some_and(char::is_ascii_digit) {
        characters.next();
        has_digit = true;
    }
    has_digit
}
